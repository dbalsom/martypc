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
;   Shared MartyPC utility routines.

%ifndef MARTY_INC
%define MARTY_INC

MARTY_PROBE_INTERRUPT       equ 2Fh
MARTY_PROBE_AX              equ 0F500h
MARTY_PROBE_BX              equ 0DEADh
MARTY_PROBE_CX              equ 0BEEFh
MARTY_PROBE_RESPONSE_AX     equ 0F5FFh
MARTY_PROBE_RESPONSE_BX     equ 4D50h

MARTY_SERVICE_FLAG_ENABLED  equ 01h
MARTY_SERVICE_FLAG_AVAILABLE equ 02h

; Service interrupt functions
; -----------------------------------------------------------------------------
FUNC_SERVICE_CTRL             equ 00h
FUNC_DEBUGGER                equ 01h
FUNC_PIT_LOGGING             equ 02h
FUNC_QUIT                    equ 03h
FUNC_FILE_TRANSFER_BEGIN     equ 04h
FUNC_FILE_TRANSFER_BLOCK     equ 05h
FUNC_FILE_TRANSFER_END       equ 06h
FUNC_SPEED_CONTROL           equ 10h

; FUNC_SPEED_CONTROL AL subfunctions
SPEED_CONTROL_QUERY          equ 00h
SPEED_CONTROL_SET            equ 01h

; FUNC_FILE_TRANSFER_BEGIN AL flags
FILE_TRANSFER_GUEST_TO_HOST   equ 00h
FILE_TRANSFER_HOST_TO_GUEST   equ 01h
FILE_TRANSFER_DIRECTION_MASK  equ 01h
FILE_TRANSFER_NON_INTERACTIVE equ 02h
FILE_TRANSFER_FLAG_MASK       equ 03h

FILE_TRANSFER_COMMIT          equ 00h
FILE_TRANSFER_ABORT           equ 01h
FILE_TRANSFER_STRUCT_SIZE     equ 0Ah
FILE_TRANSFER_STATUS_WAIT     equ 0000h
FILE_TRANSFER_STATUS_READY    equ 0001h
FILE_TRANSFER_STATUS_ABORTED  equ 0002h
FILE_TRANSFER_STATUS_HOST_FILE_NOT_FOUND equ 0003h

SERVICE_ERROR_INVALID_FUNCTION   equ 0001h
SERVICE_ERROR_FILE_NOT_FOUND     equ 0002h
SERVICE_ERROR_TOO_MANY_FILES     equ 0004h
SERVICE_ERROR_INVALID_HANDLE     equ 0006h
SERVICE_ERROR_NOT_ENOUGH_MEMORY  equ 0008h
SERVICE_ERROR_INVALID_ACCESS     equ 000Ch
SERVICE_ERROR_INVALID_DATA       equ 000Dh
SERVICE_ERROR_NOT_SUPPORTED      equ 0032h
SERVICE_ERROR_INVALID_PARAMETER  equ 0057h
SERVICE_ERROR_BUSY               equ 00AAh

; FUNC_SERVICE_CTRL inputs:
;   BX = SERVICE_CTRL_BX
;   CX = SERVICE_CTRL_CX
;   AL = SERVICE_CTRL_DISABLE, SERVICE_CTRL_ENABLE, or SERVICE_CTRL_QUERY
; Returns:
;   AL = SERVICE_CTRL_DISABLE or SERVICE_CTRL_ENABLE
SERVICE_CTRL_BX              equ 0DEADh
SERVICE_CTRL_CX              equ 0BEEFh
SERVICE_CTRL_DISABLE         equ 00h
SERVICE_CTRL_ENABLE          equ 01h
SERVICE_CTRL_QUERY           equ 02h

%include "../common/dos.inc"

; MartyPC detection routine
; -----------------------------------------------------------------------------
; This uses the standard DOS multiplex interrupt 2Fh to detect if we are
; running under MartyPC.
;
; Output:
;   CF set:   MartyPC not detected; cached service information is cleared,
;             and CX, DX, and SI are undefined
;   CF clear: MartyPC detected
;             CX = MartyPC Cargo package version
;                  MSB == major version
;                  LSB == minor version
;             DL = configured service interrupt vector (if enabled)
;             DH = service flags
;             SI = API version
;                  MSB == major version
;                  LSB == minor version
;             The returned service information is also cached in
;             marty_si_vector, marty_si_flags, marty_version, and
;             marty_api_version.
;
;             Service flags:
;               Bit 0: MartyPC service enabled
;               Bit 1: MartyPC service interrupt available
;                      If set, DL = service interrupt vector
;                      If not set, DL is undefined
;
; Clobbers: AX, BX, CX, DX, SI, and status flags.
marty_probe:
    mov     ax, MARTY_PROBE_AX
    mov     bx, MARTY_PROBE_BX
    mov     cx, MARTY_PROBE_CX
    int     MARTY_PROBE_INTERRUPT

    cmp     ax, MARTY_PROBE_RESPONSE_AX
    jne     .not_detected
    cmp     bx, MARTY_PROBE_RESPONSE_BX
    jne     .not_detected

    mov     [cs:marty_si_flags], dh
    mov     [cs:marty_version], cx
    mov     [cs:marty_api_version], si

    test    dh, MARTY_SERVICE_FLAG_AVAILABLE
    jz      .si_unavailable

    mov     [cs:marty_si_vector], dl
    mov     [cs:marty_si_operand], dl
    clc
    ret

.si_unavailable:
    mov     byte [cs:marty_si_vector], 0
    mov     byte [cs:marty_si_operand], 0
    clc
    ret

.not_detected:
    mov     byte [cs:marty_si_vector], 0
    mov     byte [cs:marty_si_flags], 0
    mov     word [cs:marty_version], 0
    mov     word [cs:marty_api_version], 0
    mov     byte [cs:marty_si_operand], 0
    stc
    ret

; Print the MartyPC version detected by marty_probe as:
;   MartyPC v<major>.<minor> detected
;
; Call marty_probe successfully before using this routine.
; Clobbers: Status flags. Preserves all registers and restores DS.
marty_print_version:
    push    ax
    push    dx
    push    ds

    push    cs
    pop     ds

    mov     dx, marty_detected_prefix
    mov     ah, 09h
    int     21h

    mov     ax, [cs:marty_version]
    mov     al, ah
    call    print_decimal_byte

    mov     dl, '.'
    mov     ah, 02h
    int     21h

    mov     al, [cs:marty_version]
    call    print_decimal_byte

    mov     dx, marty_detected_suffix
    mov     ah, 09h
    int     21h

    pop     ds
    pop     dx
    pop     ax
    ret

; Print the service API version detected by marty_probe as:
;   API version: <major>.<minor>
;
; Call marty_probe successfully before using this routine.
; Clobbers: Status flags. Preserves all registers and restores DS.
marty_print_api_version:
    push    ax
    push    dx
    push    ds

    push    cs
    pop     ds

    mov     dx, marty_api_version_prefix
    mov     ah, 09h
    int     21h

    mov     ax, [cs:marty_api_version]
    mov     al, ah
    call    print_decimal_byte

    mov     dl, '.'
    mov     ah, 02h
    int     21h

    mov     al, [cs:marty_api_version]
    call    print_decimal_byte

    mov     dx, marty_api_version_suffix
    mov     ah, 09h
    int     21h

    pop     ds
    pop     dx
    pop     ax
    ret

; MartyPC service interrupt dispatcher
; -----------------------------------------------------------------------------
; Call marty_probe before using this routine. The caller should set AH to the
; desired service function and initialize any other registers required by it.
;
; Output:
;   Service enabled:   Executes the configured service interrupt. Registers and
;                      flags contain the values returned by the service.
;   Service control:   Executes while the service is available, even if disabled.
;   Service disabled:  CF set; the interrupt is not executed.
;
; Clobbers: Status flags when the service is disabled. Otherwise determined by
;           the requested service function.
marty_service:
    test    byte [cs:marty_si_flags], MARTY_SERVICE_FLAG_AVAILABLE
    jz      marty_service_unavailable

    cmp     ah, FUNC_SERVICE_CTRL
    je      marty_service_control

    test    byte [cs:marty_si_flags], MARTY_SERVICE_FLAG_ENABLED
    jz      marty_service_unavailable
    jmp     marty_service_invoke

marty_service_control:
    cmp     bx, SERVICE_CTRL_BX
    jne     marty_service_unavailable
    cmp     cx, SERVICE_CTRL_CX
    jne     marty_service_unavailable

    call    marty_service_invoke

    cmp     al, SERVICE_CTRL_DISABLE
    je      marty_service_control_disabled
    cmp     al, SERVICE_CTRL_ENABLE
    jne     marty_service_control_done
    or      byte [cs:marty_si_flags], MARTY_SERVICE_FLAG_ENABLED
    ret

marty_service_control_disabled:
    and     byte [cs:marty_si_flags], 0FEh

marty_service_control_done:
    ret

marty_service_invoke:
    db      0CDh                    ; INT imm8
marty_si_operand:
    db      00h                     ; Patched by marty_probe
    ret

marty_service_unavailable:
    stc
    ret

; Enable MartyPC services, including when service_interrupt_gate configured
; them to start disabled.
;
; Call marty_probe successfully before using this routine.
; Output:
;   CF clear: services enabled
;   CF set:   service interrupt unavailable or enable request rejected
; Preserves all registers.
marty_enable_service:
    push    ax
    push    bx
    push    cx

    mov     ah, FUNC_SERVICE_CTRL
    mov     al, SERVICE_CTRL_ENABLE
    mov     bx, SERVICE_CTRL_BX
    mov     cx, SERVICE_CTRL_CX
    call    marty_service
    jc      .done

    cmp     al, SERVICE_CTRL_ENABLE
    je      .done
    stc

.done:
    pop     cx
    pop     bx
    pop     ax
    ret

; Disable MartyPC services.
;
; Call marty_probe successfully before using this routine.
; Output:
;   CF clear: services disabled
;   CF set:   service interrupt unavailable or disable request rejected
; Preserves all registers.
marty_disable_service:
    push    ax
    push    bx
    push    cx

    mov     ah, FUNC_SERVICE_CTRL
    mov     al, SERVICE_CTRL_DISABLE
    mov     bx, SERVICE_CTRL_BX
    mov     cx, SERVICE_CTRL_CX
    call    marty_service
    jc      .done

    cmp     al, SERVICE_CTRL_DISABLE
    je      .done
    stc

.done:
    pop     cx
    pop     bx
    pop     ax
    ret

; Update a reflected CRC-32/ISO-HDLC (CRC-32/CCITT) value for one block.
;
; Initialize DX:AX to FFFFFFFFh before the first block and complement both
; words after the final block.
;
; Input:
;   DX:AX = current unfinalized CRC-32
;   ES:SI = block data
;   CX    = block length
; Output:
;   DX:AX = updated unfinalized CRC-32
; Preserves BX, CX, and SI.
crc32_update:
    push    bx
    push    cx
    push    si

.next_byte:
    jcxz    .done
    xor     al, [es:si]
    inc     si
    mov     bx, 8

.next_bit:
    shr     dx, 1
    rcr     ax, 1
    jnc     .no_polynomial
    xor     ax, 8320h
    xor     dx, 0EDB8h

.no_polynomial:
    dec     bx
    jnz     .next_bit
    loop    .next_byte

.done:
    pop     si
    pop     cx
    pop     bx
    ret

; Cached values populated by marty_probe.
marty_si_vector:    db 0
marty_si_flags:     db 0
marty_version:      dw 0
marty_api_version:  dw 0
marty_detected_prefix: db 'MartyPC v', '$'
marty_detected_suffix: db ' detected', 0Dh, 0Ah, '$'
marty_api_version_prefix: db 'API version: ', '$'
marty_api_version_suffix: db 0Dh, 0Ah, '$'

%endif
