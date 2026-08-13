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
;
;   Shared DOS utility routines.

%ifndef DOS_INC
%define DOS_INC

; Hexadecimal output routines
; -----------------------------------------------------------------------------
; Print AL as two hexadecimal digits using DOS interrupt 21h, function 02h.
;
; Clobbers: AX, DX, and status flags. Preserves CX.
print_hex_byte:
    push    ax
    push    cx

    mov     cl, 4
    shr     al, cl
    call    print_hex_nibble

    pop     cx
    pop     ax
    and     al, 0Fh

; Print the low nibble of AL as one hexadecimal digit using DOS interrupt 21h,
; function 02h.
;
; Clobbers: AX, DX, and status flags.
print_hex_nibble:
    cmp     al, 9
    jbe     .digit
    add     al, 7
.digit:
    add     al, '0'
    mov     dl, al
    mov     ah, 02h
    int     21h
    ret

; Decimal output routines
; -----------------------------------------------------------------------------
; Print AL as an unsigned decimal value using DOS interrupt 21h, function 02h.
;
; Clobbers: Status flags. Preserves all registers.
print_decimal_byte:
    push    ax
    push    bx
    push    cx
    push    dx

    xor     ah, ah
    xor     cx, cx
    mov     bx, 10

.convert:
    xor     dx, dx
    div     bx
    push    dx
    inc     cx
    test    ax, ax
    jnz     .convert

.print:
    pop     dx
    add     dl, '0'
    mov     ah, 02h
    int     21h
    loop    .print

    pop     dx
    pop     cx
    pop     bx
    pop     ax
    ret

; Print DX:AX as an unsigned decimal value using DOS interrupt 21h, function
; 02h.
;
; Clobbers: Status flags. Preserves all registers.
print_decimal_dword:
    push    ax
    push    bx
    push    cx
    push    dx
    push    si
    push    di

    mov     bx, 10
    xor     cx, cx

.convert:
    mov     si, ax
    mov     ax, dx
    xor     dx, dx
    div     bx
    mov     di, ax
    mov     ax, si
    div     bx
    push    dx
    inc     cx
    mov     dx, di
    or      di, ax
    jnz     .convert

.print:
    pop     dx
    add     dl, '0'
    mov     ah, 02h
    int     21h
    loop    .print

    pop     di
    pop     si
    pop     dx
    pop     cx
    pop     bx
    pop     ax
    ret

; DOS error formatting
; -----------------------------------------------------------------------------
; Format an error returned by a DOS interrupt 21h file function.
;
; Input:
;   AH = DOS function number
;   AL = DOS error code
;
; Output:
;   CS:DX = address of a '$'-terminated error description
;
; Supports the file and memory functions used by msend and mrecv.
; Clobbers: DX and status flags. Preserves AX.
format_dos_error:
    cmp     ah, 3Dh
    je      .open
    cmp     ah, 3Ch
    je      .create
    cmp     ah, 3Eh
    je      .handle_only
    cmp     ah, 40h
    je      .write
    cmp     ah, 41h
    je      .delete
    cmp     ah, 42h
    je      .seek
    cmp     ah, 48h
    je      .allocate
    cmp     ah, 49h
    je      .memory_block
    cmp     ah, 4Ah
    je      .memory_block
    mov     dx, dos_error_unknown
    ret

.create:
    cmp     al, 03h
    je      .path_not_found
    cmp     al, 04h
    je      .too_many_open_files
    cmp     al, 05h
    je      .access_denied
    mov     dx, dos_error_unknown
    ret

.open:
    cmp     al, 02h
    je      .file_not_found
    cmp     al, 03h
    je      .path_not_found
    cmp     al, 04h
    je      .too_many_open_files
    cmp     al, 05h
    je      .access_denied
    cmp     al, 0Ch
    je      .invalid_access
    mov     dx, dos_error_unknown
    ret

.seek:
    cmp     al, 01h
    je      .invalid_function
    cmp     al, 06h
    je      .invalid_handle
    mov     dx, dos_error_unknown
    ret

.handle_only:
    cmp     al, 06h
    je      .invalid_handle
    mov     dx, dos_error_unknown
    ret

.write:
    cmp     al, 05h
    je      .access_denied
    cmp     al, 06h
    je      .invalid_handle
    mov     dx, dos_error_unknown
    ret

.delete:
    cmp     al, 02h
    je      .file_not_found
    cmp     al, 03h
    je      .path_not_found
    cmp     al, 05h
    je      .access_denied
    mov     dx, dos_error_unknown
    ret

.allocate:
    cmp     al, 07h
    je      .memory_control_destroyed
    cmp     al, 08h
    je      .insufficient_memory
    mov     dx, dos_error_unknown
    ret

.memory_block:
    cmp     al, 07h
    je      .memory_control_destroyed
    cmp     al, 08h
    je      .insufficient_memory
    cmp     al, 09h
    je      .invalid_memory_block
    mov     dx, dos_error_unknown
    ret

.file_not_found:
    mov     dx, dos_error_file_not_found
    ret
.path_not_found:
    mov     dx, dos_error_path_not_found
    ret
.too_many_open_files:
    mov     dx, dos_error_too_many_open_files
    ret
.access_denied:
    mov     dx, dos_error_access_denied
    ret
.invalid_access:
    mov     dx, dos_error_invalid_access
    ret
.invalid_function:
    mov     dx, dos_error_invalid_function
    ret
.invalid_handle:
    mov     dx, dos_error_invalid_handle
    ret
.memory_control_destroyed:
    mov     dx, dos_error_memory_control_destroyed
    ret
.insufficient_memory:
    mov     dx, dos_error_insufficient_memory
    ret
.invalid_memory_block:
    mov     dx, dos_error_invalid_memory_block
    ret

dos_error_unknown: db 'Unknown DOS error', '$'
dos_error_file_not_found: db 'File not found', '$'
dos_error_path_not_found: db 'Path not found', '$'
dos_error_too_many_open_files: db 'Too many open files', '$'
dos_error_access_denied: db 'Access denied', '$'
dos_error_invalid_access: db 'Invalid access code', '$'
dos_error_invalid_function: db 'Invalid function', '$'
dos_error_invalid_handle: db 'Invalid file handle', '$'
dos_error_memory_control_destroyed: db 'Memory control blocks destroyed', '$'
dos_error_insufficient_memory: db 'Insufficient memory', '$'
dos_error_invalid_memory_block: db 'Invalid memory block address', '$'

%endif
