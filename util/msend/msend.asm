;   MartyPC
;   https://github.com/dbalsom/martypc
;
;   Copyright 2022-2026 Daniel Balsom
;
;   Permission is hereby granted, free of charge, to any person obtaining a
;   copy of this software and associated documentation files (the "Software"),
;   to deal in the Software without restriction, including without limitation
;   the rights to use, copy, modify, merge, publish, distribute, sublicense,
;   and/or sell copies of the Software, and to permit persons to whom the
;   Software is furnished to do so, subject to the following conditions:
;
;   The above copyright notice and this permission notice shall be included in
;   all copies or substantial portions of the Software.
;
;   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
;   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
;   FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
;   AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
;   LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
;   FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
;   DEALINGS IN THE SOFTWARE.
;
;   ---------------------------------------------------------------------------

;   msend.asm
;
;   Send a file from the DOS guest to the host through MartyPC services.
;
;   Version 1.0.1
;     - Use the DOS-style, case-insensitive /N switch
;
;   Version 1.0.0
;     - Initial version

cpu 8086
bits 16
org 100h

section .text

start:
    cli
    mov     ax, cs
    mov     ss, ax
    mov     sp, stack_top
    sti
    mov     ds, ax
    mov     es, ax

    call    release_unused_memory
    jc      resize_failed

    call    marty_probe
    jc      martypc_not_detected
    call    marty_print_version
    call    marty_enable_service
    jc      service_enable_failed

    call    read_command_line
    jc      show_usage

    mov     dx, filename
    mov     ax, 3D00h                 ; Open existing file, read-only
    int     21h
    jc      open_failed

    mov     [file_handle], ax
    mov     bx, ax                    ; BX = DOS file handle
    mov     ax, 4202h                 ; Seek from end of file
    xor     cx, cx
    xor     dx, dx
    int     21h
    jc      seek_failed

    mov     [file_size_low], ax
    mov     [file_size_high], dx

    mov     bx, [file_handle]
    mov     ax, 4200h                 ; Seek back to the start of the file
    xor     cx, cx
    xor     dx, dx
    int     21h
    jc      seek_failed

    mov     dx, sending_prefix
    mov     ah, 09h
    int     21h

    mov     bx, 1                     ; Standard output
    mov     cx, [filename_length]
    mov     dx, filename
    mov     ah, 40h
    int     21h

    mov     dx, size_separator
    mov     ah, 09h
    int     21h

    mov     ax, [file_size_low]
    mov     dx, [file_size_high]
    call    print_decimal_dword

    mov     dx, bytes_suffix
    mov     ah, 09h
    int     21h

    call    allocate_transfer_buffer
    jc      allocation_failed
    call    calculate_block_count

    mov     ax, ds
    mov     [transfer_filename_segment], ax
    mov     es, ax
    mov     di, transfer_structure
    mov     cx, FILE_TRANSFER_STRUCT_SIZE
    mov     ah, FUNC_FILE_TRANSFER_BEGIN
    mov     al, FILE_TRANSFER_GUEST_TO_HOST
    cmp     byte [non_interactive], 0
    je      .begin_transfer
    or      al, FILE_TRANSFER_NON_INTERACTIVE
.begin_transfer:
    call    marty_service
    jc      transfer_begin_failed

    mov     [transfer_handle], bx
    mov     byte [transfer_active], 1

    mov     dx, handle_prefix
    mov     ah, 09h
    int     21h

    mov     bx, [transfer_handle]
    mov     al, bh
    call    print_hex_byte
    mov     al, bl
    call    print_hex_byte

    mov     dx, handle_block_separator
    mov     ah, 09h
    int     21h

    mov     ax, [block_count_low]
    mov     dx, [block_count_high]
    call    print_decimal_dword

    mov     dx, newline
    mov     ah, 09h
    int     21h

    mov     ax, [file_size_low]
    mov     [remaining_low], ax
    mov     ax, [file_size_high]
    mov     [remaining_high], ax

send_next_block:
    mov     ax, [remaining_low]
    or      ax, [remaining_high]
    jz      finalize_transfer

    cmp     word [remaining_high], 0
    jne     .maximum_length
    mov     cx, [remaining_low]
    jmp     .read

.maximum_length:
    mov     cx, 0FFFFh                ; Maximum API block length

.read:
    mov     bx, [file_handle]
    mov     ax, [buffer_segment]
    push    ds
    mov     ds, ax
    xor     dx, dx                    ; DS:DX = transfer buffer
    mov     ah, 3Fh                   ; Read source file
    int     21h
    pop     ds
    jc      read_failed

    test    ax, ax
    jz      unexpected_eof
    mov     [block_length], ax

    mov     cx, ax
    mov     bx, [transfer_handle]
    mov     ax, [buffer_segment]
    mov     es, ax
    xor     di, di                    ; ES:DI = transfer buffer
    mov     ah, FUNC_FILE_TRANSFER_BLOCK
    call    marty_service
    jc      transfer_block_failed

    cmp     ax, [block_length]
    jne     short_transfer

    mov     ax, [crc32_low]
    mov     dx, [crc32_high]
    xor     si, si
    mov     cx, [block_length]
    call    crc32_update
    mov     [crc32_low], ax
    mov     [crc32_high], dx

    mov     ax, [block_length]
    sub     [remaining_low], ax
    sbb     word [remaining_high], 0
    jmp     send_next_block

finalize_transfer:
    call    close_source_file

    not     word [crc32_low]
    not     word [crc32_high]

    mov     bx, [transfer_handle]
    mov     ah, FUNC_FILE_TRANSFER_END
    mov     al, FILE_TRANSFER_COMMIT
    call    marty_service
    jc      transfer_finalize_failed

    mov     byte [transfer_active], 0
    cmp     cx, [crc32_low]
    jne     crc_mismatch
    cmp     dx, [crc32_high]
    jne     crc_mismatch
    call    free_transfer_buffer
    call    marty_disable_service

    mov     ax, 4C00h
    int     21h

crc_mismatch:
    call    free_transfer_buffer
    mov     dx, crc_mismatch_msg
    jmp     error_exit

transfer_begin_failed:
    mov     [error_code], ax
    call    close_source_file
    call    free_transfer_buffer
    mov     dx, transfer_begin_failed_prefix
    jmp     service_error_exit

read_failed:
    mov     [error_code], ax
    call    abort_transfer
    call    close_source_file
    call    free_transfer_buffer
    mov     dx, read_failed_prefix
    jmp     memory_error_exit

transfer_block_failed:
    mov     [error_code], ax
    call    abort_transfer
    call    close_source_file
    call    free_transfer_buffer
    mov     dx, transfer_block_failed_prefix
    jmp     service_error_exit

transfer_finalize_failed:
    mov     [error_code], ax
    call    abort_transfer
    call    free_transfer_buffer
    mov     dx, transfer_finalize_failed_prefix
    jmp     service_error_exit

unexpected_eof:
    call    abort_transfer
    call    close_source_file
    call    free_transfer_buffer
    mov     dx, unexpected_eof_msg
    jmp     error_exit

short_transfer:
    call    abort_transfer
    call    close_source_file
    call    free_transfer_buffer
    mov     dx, short_transfer_msg
    jmp     error_exit

allocation_failed:
    mov     [error_code], ax
    call    close_source_file
    mov     dx, allocation_failed_prefix
    jmp     memory_error_exit

resize_failed:
    mov     [error_code], ax
    mov     dx, resize_failed_prefix

memory_error_exit:
    mov     ah, 09h
    int     21h
    mov     bx, [error_code]
    mov     al, bh
    call    print_hex_byte
    mov     al, bl
    call    print_hex_byte
    mov     dx, hex_error_suffix
    jmp     error_exit

service_error_exit:
    mov     ah, 09h
    int     21h
    mov     bx, [error_code]
    mov     al, bh
    call    print_hex_byte
    mov     al, bl
    call    print_hex_byte
    mov     dx, hex_error_suffix
    jmp     error_exit

seek_failed:
    mov     [error_code], ax
    call    close_source_file
    mov     ax, [error_code]
    mov     ah, 42h
    call    format_dos_error
    push    dx
    mov     dx, seek_failed_prefix
    jmp     formatted_error_exit

open_failed:
    mov     ah, 3Dh
    call    format_dos_error
    push    dx
    mov     dx, open_failed_prefix

formatted_error_exit:
    mov     ah, 09h
    int     21h
    pop     dx
    mov     ah, 09h
    int     21h
    mov     dx, newline
    jmp     error_exit

show_usage:
    mov     dx, usage_msg
    jmp     error_exit

martypc_not_detected:
    mov     dx, not_detected_msg
    jmp     error_exit

service_enable_failed:
    mov     dx, service_enable_failed_msg

error_exit:
    call    marty_disable_service
    mov     ah, 09h
    int     21h
    mov     ax, 4C01h
    int     21h

; Release the unused portion of the memory block DOS assigned to this COM file.
; The stack has already been moved inside the retained portion of the block.
;
; Output:
;   CF clear: memory block resized
;   CF set:   AX = DOS error code
release_unused_memory:
    mov     ax, ds
    mov     es, ax
    mov     bx, program_end
    add     bx, 15
    mov     cl, 4
    shr     bx, cl
    mov     ah, 4Ah
    int     21h
    ret

; Allocate a paragraph-aligned transfer buffer. The requested size is the file
; size rounded up to a paragraph, capped at 64 KiB. Empty files receive one
; paragraph because DOS cannot allocate a zero-length block.
;
; Output:
;   CF clear: buffer_segment populated
;   CF set:   AX = DOS error code
allocate_transfer_buffer:
    cmp     word [file_size_high], 0
    jne     .maximum

    mov     ax, [file_size_low]
    mov     bx, ax
    mov     cl, 4
    shr     bx, cl
    test    al, 0Fh
    jz      .check_empty
    inc     bx

.check_empty:
    test    bx, bx
    jnz     .allocate
    mov     bx, 1
    jmp     .allocate

.maximum:
    mov     bx, 1000h                ; 4096 paragraphs = 64 KiB

.allocate:
    mov     ah, 48h
    int     21h
    jc      .done
    mov     [buffer_segment], ax

.done:
    ret

; Calculate the number of transfer blocks required for the file. Although the
; allocated buffer is 64 KiB, the API's 16-bit length is capped at FFFFh, so
; the block count is ceil(file size / 65535). Empty files report one block.
;
; Output:
;   block_count_high:block_count_low = number of blocks
; Preserves all registers.
calculate_block_count:
    push    ax
    push    bx
    push    dx

    mov     bx, 0FFFFh
    mov     ax, [file_size_high]
    xor     dx, dx
    div     bx
    mov     [block_count_high], ax

    mov     ax, [file_size_low]
    div     bx
    mov     [block_count_low], ax

    test    dx, dx                    ; Round up if a remainder exists
    jz      .minimum
    add     word [block_count_low], 1
    adc     word [block_count_high], 0

.minimum:
    mov     ax, [block_count_low]
    or      ax, [block_count_high]
    jnz     .done
    mov     word [block_count_low], 1

.done:
    pop     dx
    pop     bx
    pop     ax
    ret

; Close the source file if it is open.
; Preserves all registers and ignores a DOS close error during cleanup.
close_source_file:
    push    ax
    push    bx

    mov     bx, [file_handle]
    cmp     bx, 0FFFFh
    je      .done
    mov     ah, 3Eh
    int     21h
    mov     word [file_handle], 0FFFFh

.done:
    pop     bx
    pop     ax
    ret

; Abort the active transfer during error cleanup.
; Preserves all registers and ignores a service error during cleanup.
abort_transfer:
    cmp     byte [transfer_active], 0
    je      .done

    push    ax
    push    bx

    mov     bx, [transfer_handle]
    mov     ah, FUNC_FILE_TRANSFER_END
    mov     al, FILE_TRANSFER_ABORT
    call    marty_service
    mov     byte [transfer_active], 0

    pop     bx
    pop     ax

.done:
    ret

; Free the transfer buffer if it was allocated.
; Preserves all registers and ignores a DOS free error during shutdown.
free_transfer_buffer:
    push    ax
    push    es

    mov     ax, [buffer_segment]
    test    ax, ax
    jz      .done
    mov     es, ax
    mov     ah, 49h
    int     21h
    mov     word [buffer_segment], 0

.done:
    pop     es
    pop     ax
    ret

; Read the first command-line argument into filename as an ASCIIZ string.
; Quoted paths may contain spaces. Unquoted paths end at the first space or tab.
;
; Output:
;   CF clear: filename and filename_length populated
;   CF set:   no filename supplied
;
; Preserves all registers.
read_command_line:
    push    ax
    push    bx
    push    cx
    push    si
    push    di

    mov     si, 81h
    xor     cx, cx
    mov     cl, [80h]

.skip_whitespace:
    jcxz    .failed
    lodsb
    dec     cx
    cmp     al, ' '
    je      .skip_whitespace
    cmp     al, 09h
    je      .skip_whitespace

    ; Recognize an optional leading /N switch as a complete token.
    cmp     al, '/'
    jne     .begin_filename
    jcxz    .begin_filename
    mov     ah, [si]
    cmp     ah, 'N'
    je      .check_switch_end
    cmp     ah, 'n'
    jne     .begin_filename

.check_switch_end:
    cmp     cx, 1
    je      .enable_non_interactive
    mov     ah, [si + 1]
    cmp     ah, ' '
    je      .enable_non_interactive
    cmp     ah, 09h
    jne     .begin_filename

.enable_non_interactive:
    inc     si                       ; Consume the N in /N
    dec     cx
    mov     byte [non_interactive], 1
    jmp     .skip_whitespace

.begin_filename:
    mov     di, filename
    xor     bx, bx
    cmp     al, '"'
    je      .copy_quoted

.copy_unquoted:
    stosb
    inc     bx
    jcxz    .finished
    lodsb
    dec     cx
    cmp     al, ' '
    je      .finished
    cmp     al, 09h
    jne     .copy_unquoted
    jmp     .finished

.copy_quoted:
    jcxz    .finished
    lodsb
    dec     cx
    cmp     al, '"'
    je      .finished
    stosb
    inc     bx
    jmp     .copy_quoted

.finished:
    test    bx, bx
    jz      .failed
    mov     byte [di], 0
    mov     [filename_length], bx
    clc
    jmp     .done

.failed:
    stc

.done:
    pop     di
    pop     si
    pop     cx
    pop     bx
    pop     ax
    ret

%include "../common/marty.inc"

section .data

not_detected_msg            db 'MartyPC not detected', 0Dh, 0Ah, '$'
service_enable_failed_msg   db 'Unable to enable MartyPC services', 0Dh, 0Ah, '$'
usage_msg                   db 'Usage: MSEND [/N] <file>', 0Dh, 0Ah, '$'
open_failed_prefix          db 'Unable to open source file: ', '$'
seek_failed_prefix          db 'Unable to determine source file size: ', '$'
resize_failed_prefix        db 'Unable to resize program memory block. DOS error: ', '$'
allocation_failed_prefix    db 'Unable to allocate transfer buffer. DOS error: ', '$'
transfer_begin_failed_prefix      db 'Unable to begin file transfer. Service error: ', '$'
read_failed_prefix                db 'Unable to read source file. DOS error: ', '$'
transfer_block_failed_prefix      db 'Unable to send transfer block. Service error: ', '$'
transfer_finalize_failed_prefix   db 'Unable to finalize file transfer. Service error: ', '$'
unexpected_eof_msg          db 'Unexpected end of source file', 0Dh, 0Ah, '$'
short_transfer_msg          db 'MartyPC accepted only part of a transfer block', 0Dh, 0Ah, '$'
crc_mismatch_msg            db 'File transfer CRC-32 mismatch', 0Dh, 0Ah, '$'
sending_prefix              db 'Sending ', '$'
size_separator              db ', ', '$'
bytes_suffix                db ' bytes...', 0Dh, 0Ah, '$'
handle_prefix               db 'Transfer handle: ', '$'
handle_block_separator      db 'h, blocks: ', '$'
hex_error_suffix            db 'h', 0Dh, 0Ah, '$'
newline                     db 0Dh, 0Ah, '$'

filename_length             dw 0
transfer_structure:
transfer_filename_offset    dw filename
transfer_filename_segment   dw 0
file_size_low               dw 0
file_size_high              dw 0
transfer_status             dw FILE_TRANSFER_STATUS_READY
buffer_segment              dw 0
file_handle                 dw 0FFFFh
transfer_handle             dw 0
transfer_active             db 0
non_interactive             db 0
block_length                dw 0
block_count_low             dw 0
block_count_high            dw 0
remaining_low               dw 0
remaining_high              dw 0
crc32_low                   dw 0FFFFh
crc32_high                  dw 0FFFFh
error_code                  dw 0
filename                    times 128 db 0
stack_space                 times 256 db 0
stack_top:
program_end:
