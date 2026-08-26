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

;   mrecv.asm
;
;   Receive a host file into the DOS guest through MartyPC services.
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
    call    initialize_transfer_filename

    mov     dx, destination_filename
    mov     ax, 4300h                 ; Get destination file attributes
    int     21h
    jc      create_destination        ; File not found

    mov     dx, overwrite_prefix
    mov     ah, 09h
    int     21h

    mov     bx, 1                     ; Standard output
    mov     cx, [destination_filename_length]
    mov     dx, destination_filename
    mov     ah, 40h
    int     21h

    mov     dx, overwrite_suffix
    mov     ah, 09h
    int     21h

confirm_overwrite:
    mov     ah, 01h                   ; Read and echo one character
    int     21h
    cmp     al, 'Y'
    je      overwrite_confirmed
    cmp     al, 'y'
    je      overwrite_confirmed
    cmp     al, 'N'
    je      overwrite_declined
    cmp     al, 'n'
    jne     confirm_overwrite

overwrite_declined:
    mov     dx, newline
    mov     ah, 09h
    int     21h
    call    marty_disable_service
    mov     ax, 4C00h
    int     21h

overwrite_confirmed:
    mov     dx, newline
    mov     ah, 09h
    int     21h

create_destination:
    xor     cx, cx                    ; Normal file attributes
    mov     dx, destination_filename
    mov     ah, 3Ch                   ; Create or truncate before host selection
    int     21h
    jc      create_failed

    mov     [file_handle], ax
    mov     byte [output_created], 1

    mov     ax, ds
    mov     [transfer_filename_segment], ax

request_transfer:
    mov     es, ax
    mov     di, transfer_structure
    mov     cx, FILE_TRANSFER_STRUCT_SIZE
    mov     ah, FUNC_FILE_TRANSFER_BEGIN
    mov     al, FILE_TRANSFER_HOST_TO_GUEST
    cmp     byte [non_interactive], 0
    je      .begin_transfer
    or      al, FILE_TRANSFER_NON_INTERACTIVE
.begin_transfer:
    call    marty_service
    jc      transfer_begin_failed

    mov     [transfer_handle], bx
    mov     byte [transfer_active], 1

wait_for_file:
    mov     ax, [transfer_status]
    cmp     ax, FILE_TRANSFER_STATUS_READY
    je      transfer_ready
    cmp     ax, FILE_TRANSFER_STATUS_ABORTED
    je      transfer_cancelled
    cmp     ax, FILE_TRANSFER_STATUS_HOST_FILE_NOT_FOUND
    je      host_file_not_found
    cmp     ax, FILE_TRANSFER_STATUS_WAIT
    jne     invalid_transfer_status
    int     28h                       ; Yield while the host dialog is open
    jmp     wait_for_file

transfer_ready:
    mov     ax, [file_size_low]
    or      ax, [file_size_high]
    jz      empty_file

    call    calculate_source_filename_length

    mov     dx, transferring_prefix
    mov     ah, 09h
    int     21h

    mov     bx, 1                     ; Standard output
    mov     cx, [source_filename_length]
    mov     dx, source_filename
    mov     ah, 40h
    int     21h

    mov     dx, transfer_separator
    mov     ah, 09h
    int     21h

    mov     bx, 1                     ; Standard output
    mov     cx, [destination_filename_length]
    mov     dx, destination_filename
    mov     ah, 40h
    int     21h

    mov     dx, transfer_suffix
    mov     ah, 09h
    int     21h

    mov     dx, size_prefix
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

receive_next_block:
    mov     ax, [remaining_low]
    or      ax, [remaining_high]
    jz      finalize_transfer

    cmp     word [remaining_high], 0
    jne     .maximum_length
    mov     cx, [remaining_low]
    jmp     .transfer

.maximum_length:
    mov     cx, 0FFFFh                ; Maximum API block length

.transfer:
    mov     bx, [transfer_handle]
    mov     ax, [buffer_segment]
    mov     es, ax
    xor     di, di                    ; ES:DI = transfer buffer
    mov     ah, FUNC_FILE_TRANSFER_BLOCK
    call    marty_service
    jc      transfer_block_failed

    test    ax, ax
    jz      unexpected_eof
    mov     [block_length], ax

    mov     cx, ax
    mov     bx, [file_handle]
    mov     ax, [buffer_segment]
    push    ds
    mov     ds, ax
    xor     dx, dx                    ; DS:DX = transfer buffer
    mov     ah, 40h                   ; Write destination file
    int     21h
    pop     ds
    jc      write_failed

    cmp     ax, [block_length]
    jne     short_write

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
    jmp     receive_next_block

finalize_transfer:
    mov     bx, [file_handle]
    mov     ah, 3Eh
    int     21h
    jc      close_failed
    mov     word [file_handle], 0FFFFh

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
    mov     byte [output_created], 0
    call    free_transfer_buffer
    call    marty_disable_service

    mov     dx, complete_msg
    mov     ah, 09h
    int     21h
    mov     ax, 4C00h
    int     21h

crc_mismatch:
    mov     byte [output_created], 0
    call    free_transfer_buffer
    mov     dx, crc_mismatch_msg
    mov     ah, 09h
    int     21h
    call    marty_disable_service
    mov     ax, 4C01h
    int     21h

transfer_begin_failed:
    mov     [error_code], ax
    call    cleanup_output_file
    mov     dx, transfer_begin_failed_prefix
    jmp     service_error_exit

transfer_cancelled:
    call    abort_transfer
    call    cleanup_output_file
    mov     dx, transfer_cancelled_msg
    jmp     error_exit

host_file_not_found:
    call    abort_transfer
    call    cleanup_output_file
    mov     dx, host_file_not_found_msg
    jmp     error_exit

invalid_transfer_status:
    call    abort_transfer
    call    cleanup_output_file
    mov     dx, invalid_transfer_status_msg
    jmp     error_exit

empty_file:
    call    abort_transfer
    call    cleanup_output_file
    mov     dx, empty_file_msg
    jmp     error_exit

allocation_failed:
    mov     [error_code], ax
    call    abort_transfer
    call    cleanup_output_file
    mov     ax, [error_code]
    mov     ah, 48h
    call    format_dos_error
    push    dx
    mov     dx, allocation_failed_prefix
    jmp     formatted_error_exit

create_failed:
    mov     [error_code], ax
    mov     ax, [error_code]
    mov     ah, 3Ch
    call    format_dos_error
    push    dx
    mov     dx, create_failed_prefix
    jmp     formatted_error_exit

write_failed:
    mov     [error_code], ax
    call    abort_transfer
    call    cleanup_output_file
    call    free_transfer_buffer
    mov     ax, [error_code]
    mov     ah, 40h
    call    format_dos_error
    push    dx
    mov     dx, write_failed_prefix
    jmp     formatted_error_exit

close_failed:
    mov     [error_code], ax
    call    abort_transfer
    call    cleanup_output_file
    call    free_transfer_buffer
    mov     ax, [error_code]
    mov     ah, 3Eh
    call    format_dos_error
    push    dx
    mov     dx, close_failed_prefix
    jmp     formatted_error_exit

transfer_block_failed:
    mov     [error_code], ax
    call    abort_transfer
    call    cleanup_output_file
    call    free_transfer_buffer
    mov     dx, transfer_block_failed_prefix
    jmp     service_error_exit

transfer_finalize_failed:
    mov     [error_code], ax
    call    abort_transfer
    call    cleanup_output_file
    call    free_transfer_buffer
    mov     dx, transfer_finalize_failed_prefix
    jmp     service_error_exit

unexpected_eof:
    call    abort_transfer
    call    cleanup_output_file
    call    free_transfer_buffer
    mov     dx, unexpected_eof_msg
    jmp     error_exit

short_write:
    call    abort_transfer
    call    cleanup_output_file
    call    free_transfer_buffer
    mov     dx, short_write_msg
    jmp     error_exit

resize_failed:
    mov     [error_code], ax
    mov     ah, 4Ah
    call    format_dos_error
    push    dx
    mov     dx, resize_failed_prefix
    jmp     formatted_error_exit

formatted_error_exit:
    mov     ah, 09h
    int     21h
    pop     dx
    mov     ah, 09h
    int     21h
    mov     dx, newline
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

martypc_not_detected:
    mov     dx, not_detected_msg
    jmp     error_exit

service_enable_failed:
    mov     dx, service_enable_failed_msg
    jmp     error_exit

show_usage:
    mov     dx, usage_msg

error_exit:
    call    marty_disable_service
    mov     ah, 09h
    int     21h
    mov     ax, 4C01h
    int     21h

; Release the unused portion of the memory block DOS assigned to this COM file.
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

; Allocate a transfer buffer sized to the file, capped at 64 KiB.
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
    mov     bx, 1000h

.allocate:
    mov     ah, 48h
    int     21h
    jc      .done
    mov     [buffer_segment], ax

.done:
    ret

; Calculate ceil(file size / 65535). Empty files report one block.
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

    test    dx, dx
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

; Read the first command-line argument as the DOS destination filename.
; Quoted paths may contain spaces. Unquoted paths end at a space or tab.
;
; Output:
;   CF clear: destination_filename and length populated
;   CF set:   no destination filename supplied
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

    ; Recognize an optional leading -n switch as a complete token.
    cmp     al, '-'
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
    inc     si                       ; Consume the n in -n
    dec     cx
    mov     byte [non_interactive], 1
    jmp     .skip_whitespace

.begin_filename:
    mov     di, destination_filename
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
    mov     [destination_filename_length], bx
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

; Initialize the transfer structure's filename buffer with the destination
; filename supplied to mrecv. MartyPC may replace this buffer with host-file
; metadata after processing BEGIN; the DOS destination remains unchanged.
;
; Preserves all registers.
initialize_transfer_filename:
    push    cx
    push    si
    push    di

    mov     si, destination_filename
    mov     di, source_filename
    mov     cx, [destination_filename_length]
    inc     cx                       ; Include the ASCIIZ terminator
    cld
    rep     movsb

    pop     di
    pop     si
    pop     cx
    ret

; Determine the length of the ASCIIZ source filename returned by MartyPC.
calculate_source_filename_length:
    push    ax
    push    cx
    push    di

    xor     ax, ax
    mov     cx, 256
    mov     di, source_filename
    cld
    repne   scasb
    mov     ax, 256
    sub     ax, cx
    dec     ax
    mov     [source_filename_length], ax

    pop     di
    pop     cx
    pop     ax
    ret

; Abort the active service transfer during error cleanup.
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

; Close and delete a partial destination file.
cleanup_output_file:
    push    ax
    push    bx
    push    dx

    mov     bx, [file_handle]
    cmp     bx, 0FFFFh
    je      .delete
    mov     ah, 3Eh
    int     21h
    mov     word [file_handle], 0FFFFh

.delete:
    cmp     byte [output_created], 0
    je      .done
    mov     dx, destination_filename
    mov     ah, 41h
    int     21h
    mov     byte [output_created], 0

.done:
    pop     dx
    pop     bx
    pop     ax
    ret

; Free the transfer buffer if it was allocated.
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

%include "../common/marty.inc"

section .data

not_detected_msg            db 'MartyPC not detected', 0Dh, 0Ah, '$'
service_enable_failed_msg   db 'Unable to enable MartyPC services', 0Dh, 0Ah, '$'
usage_msg                   db 'Usage: mrecv [-n] <destination>', 0Dh, 0Ah, '$'
overwrite_prefix            db 'File ', '$'
overwrite_suffix            db ' already exists. Overwrite (y/n)? ', '$'
transferring_prefix         db 'Transferring "', '$'
transfer_separator          db '" -> "', '$'
transfer_suffix             db '"', 0Dh, 0Ah, '$'
size_prefix                 db 'Size: ', '$'
bytes_suffix                db ' bytes...', 0Dh, 0Ah, '$'
handle_prefix               db 'Transfer handle: ', '$'
handle_block_separator      db 'h, blocks: ', '$'
complete_msg                db 'Transfer complete', 0Dh, 0Ah, '$'
transfer_cancelled_msg      db 'File selection cancelled', 0Dh, 0Ah, '$'
host_file_not_found_msg     db 'File not found on host', 0Dh, 0Ah, '$'
invalid_transfer_status_msg db 'Invalid file transfer status', 0Dh, 0Ah, '$'
empty_file_msg              db 'Host file is empty', 0Dh, 0Ah, '$'
resize_failed_prefix        db 'Unable to resize program memory block: ', '$'
allocation_failed_prefix    db 'Unable to allocate transfer buffer: ', '$'
create_failed_prefix        db 'Unable to create destination file: ', '$'
write_failed_prefix         db 'Unable to write destination file: ', '$'
close_failed_prefix         db 'Unable to close destination file: ', '$'
transfer_begin_failed_prefix      db 'Unable to begin file transfer. Service error: ', '$'
transfer_block_failed_prefix      db 'Unable to receive transfer block. Service error: ', '$'
transfer_finalize_failed_prefix   db 'Unable to finalize file transfer. Service error: ', '$'
unexpected_eof_msg          db 'Unexpected end of host file', 0Dh, 0Ah, '$'
short_write_msg             db 'Unable to write the complete block; disk may be full', 0Dh, 0Ah, '$'
crc_mismatch_msg            db 'Warning: file transfer CRC-32 mismatch', 0Dh, 0Ah, '$'
hex_error_suffix            db 'h', 0Dh, 0Ah, '$'
newline                     db 0Dh, 0Ah, '$'

destination_filename_length dw 0
source_filename_length      dw 0
transfer_structure:
transfer_filename_offset    dw source_filename
transfer_filename_segment   dw 0
file_size_low               dw 0
file_size_high              dw 0
transfer_status             dw FILE_TRANSFER_STATUS_WAIT
buffer_segment              dw 0
file_handle                 dw 0FFFFh
transfer_handle             dw 0
transfer_active             db 0
non_interactive             db 0
output_created              db 0
block_length                dw 0
block_count_low             dw 0
block_count_high            dw 0
remaining_low               dw 0
remaining_high              dw 0
crc32_low                   dw 0FFFFh
crc32_high                  dw 0FFFFh
error_code                  dw 0
destination_filename        times 128 db 0
source_filename             times 256 db 0
stack_space                 times 256 db 0
stack_top:
program_end:
