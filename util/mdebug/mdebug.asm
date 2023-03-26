;
;   Marty PC Emulator 
;   (C)2023 Daniel Balsom
;   https://github.com/dbalsom/marty
;
;   This program is free software: you can redistribute it and/or modify
;   it under the terms of the GNU General Public License as published by
;   the Free Software Foundation, either version 3 of the License, or
;   (at your option) any later version.
;
;   This program is distributed in the hope that it will be useful,
;   but WITHOUT ANY WARRANTY; without even the implied warranty of
;   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
;   GNU General Public License for more details.
;
;   You should have received a copy of the GNU General Public License
;   along with this program.  If not, see <https://www.gnu.org/licenses/>.
;
;   ---------------------------------------------------------------------------
;
;   mdebug.asm
;  
;   Version 0.1
;
;   This utility will launch the executable name given on the command line as
;   a debugger using DOS int21h 4Bh with al==1. It will then call an internal
;   service interrupt to provide the emulator with the new processeses CS:IP.
;   The emulator will then jump to this address and pause execution.
;
;   Compile with nasm.  nasm mdebug.asm -o mdebug.com

cpu	8086
org	100h

section .text

start:
    
    mov   sp, 2000h;                     ; set up stack
    mov   ax, cs
    mov   ss, ax
    mov   ds, ax
    mov   es, ax                        ; Set ES=DS=CS
    
    mov   ah, 4ah
    mov   bx, 1000h
    int   21h                           ; COM files are given all memory. Resize ourselves 
                                        ; so we can load our target process.
    
    mov   bl, byte [80h]                ; Get cmdline.len
    dec   bl                            ; Length of commandline includes leading space
    mov   byte [cmdline_len], bl        ; Save length of commandline
    
    cmp   bl, 7Eh                       ; Is cmdline.len > 126?
    ja    exit                          ; Prevent overflow

;    mov   di, 81h                       ; Point DI at start of commandline
;    mov   al, 20h                       ; ascii space
;trim_spaces:                      
;    scasb                               ; is character of cmdline a space?
;    jnz   found_char                    ; jump if character was not a space
;    dec   word [cmdline_len]            ; decrement length of cmdline
;    loop  trim_spaces             
;                                  
;found_char:
;    dec   di                            ; di now points to actual start of cmdline


    ;mov   al, [82h]
    ;call  printhexb
    
    xor   cx, cx
    mov   cl, 0
    mov   cl, byte [cmdline_len]        ; Size of cmdline to cx
    mov   si, 82h                       ; SI to start of first argument
    mov   di, fn_buf                    ; DI to offset of fn_buffer
    cld
    rep   movsb                         ; Copy cmdline to buffer
    
    xor   bx, bx
    mov   bl, [cmdline_len]
    mov   byte [bx+82h], '$'            ; Terminate the command-line argument with $
    mov   ah, 09h
    mov   dx, executing
    int   21h                           ; Print string
    mov   dx, 82h
    int   21h                           ; Print string
    mov   dx, nl
    int   21h
    
execute:
    mov   ax, ds
    mov   word [pb_cmdline_seg], ax     ; Fill out data segment for command line param
    mov   ax, exec_cmdline
    mov   word [pb_cmdline_offset], ax  ; Fill out offset for command line param
    mov   ah, 4bh                       ; EXEC/Load and Execute Program
    mov   al, 01h                       ; Create program segment prefix but don't execute
    mov   dx, fn_buf                    ; Point to filename
    mov   bx, pb_env_seg                ; ES:BX to parameter block
    clc
    int   21h                           ; Execute
    
    jnc   success                       ; Process will return here when terminated
    jmp   fail
    
success:

    mov   ah, [exec_once_flag]
    cmp   ah, 01h
    jz    exit                          ; Quit if we already executed the program
    mov   byte [exec_once_flag], 01h
    
    mov   ah, 09h
    mov   dx, execute_ok
    int   21h
    
    mov   ah, 01h                       ; Emulator service 01h = debug program
    mov   bx, [pb_cs]
    mov   cx, [pb_ip]
    int   0fch                          ; Emulator service interrupt

    jmp   exit

fail:
    push  ax
    mov   ah, 09h
    mov   dx, failed_to_exec
    int   21h
    mov   dx, nl
    int   21h
    mov   dx, error_code
    int   21h
    
    pop   ax                            ; Retrieve error code
    call  printhexb
    
    
    mov   ah, 4ch
    mov   al, 01h                       ; Set error return status
    int   21h                           ; Terminate

exit:
    mov   ah, 4ch
    mov   al, 00h
    int   21h                           ; Terminate


; Prints AL in hex.
printhexb:
    push  ax
    mov   cl, 0x04
    shr   al, cl
    call  print_nibble
    pop   ax
    and   al, 0x0F
    call  print_nibble
    ret
print_nibble:
    cmp   al, 0x09
    jg    .letter
    add   al, 0x30
    mov   ah, 0x0E
    int   0x10
    ret
.letter:
    add   al, 0x37
    mov   ah, 0x0E
    int   0x10
    ret

section .data

nl                      db `\n\r$`
executing               db `Executing:$`
failed_to_exec          db `Failed to execute process!$`
error_code              db `Error code: $`
execute_ok              db `\n\rExecuted process!\n\r$`

cmdline_len             dw 0
filename                db `hello.com`,0
fn_buf                  times 128 db 0
exec_cmdline            db 1, ` `, 0
exec_once_flag          db 0
pb_env_seg              dw 0000h
pb_cmdline_offset       dw 0000h
pb_cmdline_seg          dw 0000h
pb_fcb1                 dd 0ffffffffh
pb_fcb2                 dd 0ffffffffh
pb_sp                   dw 0000h
pb_ss                   dw 0000h
pb_ip                   dw 0000h
pb_cs                   dw 0000h

