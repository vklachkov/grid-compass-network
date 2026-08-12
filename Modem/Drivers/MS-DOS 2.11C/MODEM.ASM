                .8086
                .model small

; ===========================================================================

; Segment type: Pure code
seg000          segment byte public 'CODE'
                assume cs:seg000
                assume es:nothing, ss:seg000, ds:nothing
NextDevice_0    dw offset NextDevice_1
                dw 0
DevAttr_0       dw 8000h                ; character device
Strategy_0      dw offset COM2_STRATEGY
Interrupt_0     dw offset COM2_INT
DeviceName_0    db 'COM2    '
NextDevice_1    dw 0FFFFh               ; DATA XREF: seg000:NextDevice_0↑o
                dw 0FFFFh
DevAttr_1       dw 8000h                ; character device
Strategy_1      dw offset COM2_STRATEGY
Interrupt_1     dw offset COM2STAT_INT
DeviceName_1    db 'COM2STAT'
aDosModemDriver db ' DOS modem driver version A.1.0'
                db 90h
SHADOW_MODEM_OPTIONS db 0                    ; UNK_0 ; DATA XREF: RESET+3A↓o
                                        ; MODEM_INT_HANDLER+54↓o ...
                db 0Eh                  ; BAUD_RATE_AND_DELAY
                db 52h                  ; BITS_AND_PARITY
                                        ; Data bits, stop bits, parity
                db 0                    ; UNK_3
                db 0                    ; UNK_4
                db 0                    ; UNK_5
                db 8                    ; UNK_6
                db 0                    ; UNK_7
                db 7                    ; UNK_8
DEFAULT_MODEM_OPTIONS db 0                    ; UNK_0 ; DATA XREF: RESET+37↓o
                db 0Eh                  ; BAUD_RATE_AND_DELAY
                db 52h                  ; BITS_AND_PARITY
                                        ; Data bits, stop bits, parity
                db 0                    ; UNK_3
                db 0                    ; UNK_4
                db 0                    ; UNK_5
                db 8                    ; UNK_6
                db 0                    ; UNK_7
                db 7                    ; UNK_8
REG_S9          dw 600                  ; DATA XREF: RESET+8B↓w seg000:S9_READ↓r ...
                                        ; Time in milliseconds to wait for a carrier tone.
                db 2 dup(0)
word_5A         dw 7D0h                 ; DATA XREF: RESET+85↓w
                                        ; MODEM_SET_HANGUP_TIMERS+7↓w
READ_CHAR_TIMEOUT dw 700                ; DATA XREF: COM2_INPUT_IMPL:loc_50D↓r
                                        ; INT82H_OP_SET_READ_CHAR_TIMEOUT+1↓w ...
REG_S7          dw 30                   ; DATA XREF: RESET+7F↓w seg000:S7_READ↓r ...
                                        ; Time in seconds to await a carrier signal from a distant modem.
REG_S10         dw 700                  ; DATA XREF: RESET+94↓w
                                        ; seg000:S10_READ↓r ...
                                        ; Time in milliseconds to wait before disconnecting after a remote hangs up.
REG_S8          dw 2000                 ; DATA XREF: seg000:10B4↓r seg000:120B↓w ...
                                        ; Time in milliseconds to pause when a comma is inserted in a dial command.
REG_S6          dw 2000                 ; DATA XREF: seg000:10A1↓r seg000:11F2↓w ...
                                        ; Time in milliseconds between picking up the telephone and dialing the first number.
REG_S11         dw 70                   ; DATA XREF: RESET+C2↓w
                                        ; seg000:S11_READ↓r ...
                                        ; Speed of the touch-tone dialer. In milliseconds.
word_68         dw 0                    ; DATA XREF: COM2STAT_BUILD_STATUS_REPLY+5↓o
                                        ; INT82H_OP_READ_STATUS+18↓r
word_6A         dw 0                    ; DATA XREF: INT82H_OP_READ_STATUS+14↓r
                db 0Dh
byte_6D         db 0                    ; DATA XREF: COM2STAT_INPUT_IMPL+3↓r
                                        ; COM2STAT_INPUT_IMPL:loc_62E↓w
byte_6E         db 0                    ; DATA XREF: RESET+A8↓w
                                        ; PROCESS_INPUT_D+29↓w ...
VOLUME_VALUE    db 75                   ; DATA XREF: RESET+29↓w
                                        ; INT82H_OP_SET_VOLUME:SET_VOLUME↓w ...
REG_S5          db 8                    ; DATA XREF: RESET+D1↓w seg000:S5_READ↓r ...
                                        ; ASCII value for the backspace character.
REG_S3          db 0Dh                  ; DATA XREF: RESET+D6↓w seg000:S3_READ↓r ...
                                        ; Value of the carriage return.
REG_S4          db 0Ah                  ; DATA XREF: RESET+DC↓w seg000:S4_READ↓r ...
                                        ; ASCII value for the line feed character.
REG_S2          db '+'                  ; DATA XREF: RESET+B0↓w seg000:S2_READ↓r ...
                                        ; ASCII value of the Hayes escape code.
ESCAPE_CHAR_COUNT db 0                  ; DATA XREF: RESET+AB↓w
                                        ; DETECT_HAYES_ESCAPE_SEQUENCE+11↓r ...
byte_75         db 0                    ; DATA XREF: PROCESS_PHONE_NUMBER+C↓w
                                        ; EMIT_DIGIT+25↓r ...
a0123456789Pt   db '0123456789*#.()-^ PT' ; DATA XREF: PROCESS_INPUT_D+19↓o
MODEM_STATE_BITS dw 0                   ; DATA XREF: IS_STATE_100h_FLAG↓o
                                        ; COM2_OUTPUT_IMPL:loc_59E↓o ...
STATE_BITS      dw 8000h                ; DATA XREF: COM2_OUTPUT_IMPL+3↓o
                                        ; COM2_OUTPUT_IMPL+32↓r ...
HAYES_STATE     dw 0                    ; DATA XREF: COM2_OUTPUT_IMPL+5C↓r
                                        ; RESET+9A↓w ...
                                        ; ; 08th bit -- Echo
                                        ; ; 12th bit -- Quiet
LAST_COM2_OUTPUT_TIME dd 0              ; DATA XREF: RESET+C8↓w
                                        ; UPDATE_LAST_COM2_OUTPUT_TIMESTAMP+3↓w ...
REG_S12         dw 1000                 ; DATA XREF: RESET+B6↓w
                                        ; seg000:S12_READ↓r ...
                                        ; Escape guard time in milliseconds.
word_96         dw 0                    ; DATA XREF: RESET+A5↓w seg000:10DC↓r ...
word_98         dw 0                    ; DATA XREF: START_HANDSHAKE_AND_WAIT_CONNECT+68↓w
                                        ; START_HANDSHAKE_AND_WAIT_CONNECT+75↓w ...
word_9A         dw 0                    ; DATA XREF: START_HANDSHAKE_AND_WAIT_CONNECT:loc_13BE↓w
                                        ; START_HANDSHAKE_AND_WAIT_CONNECT+79↓w ...
DTMF_TABLE      db 3Dh, 76h, 75h, 73h, 6Eh, 6Dh, 6Bh, 5Eh, 5Dh, 5Bh, 3Bh
                                        ; DATA XREF: EMIT_DIGIT+D↓o
                db 3Eh, 7Fh, 7Fh, 7Fh, 7Fh
REG_S1          dw 0                    ; DATA XREF: MODEM_INT_HANDLER+90↓w
                                        ; RESET+9F↓w ...
                                        ; Keeps a count of the number of rings on incoming calls.
RXBUF_FARPTR_OFFSET dw 0                ; DATA XREF: COM2_REQ_NDESTRUCTIVE_INPUT:loc_4B8↓r
                                        ; COM2_INPUT_IMPL+35↓r ...
RXBUF_FARPTR_SEG dw 0                   ; DATA XREF: INT82H_OP_SET_RX_BUFFER+18↓w
                                        ; RESET+5C↓w
RXBUF_SIZE      dw 0                    ; DATA XREF: COM2_REQ_NDESTRUCTIVE_INPUT+C↓r
                                        ; COM2_INPUT_IMPL:loc_531↓r ...
RXBUF_WRITE_CURSOR dw 0                 ; DATA XREF: COM2_REQ_NDESTRUCTIVE_INPUT+5↓r
                                        ; IS_RX_BUFFER_EMPTY+5↓r ...
RXBUF_READ_CURSOR dw 0                  ; DATA XREF: COM2_REQ_NDESTRUCTIVE_INPUT+1↓r
                                        ; IS_RX_BUFFER_EMPTY+1↓r ...
                align 10h
MODEM_DATA_C0   db 0                    ; DATA XREF: MODEM_READ_STATUS+1A↓w
                                        ; MODEM_READ_STATUS+35↓r ...
                align 2
MODEM_CONTROL_C2 db 0                   ; DATA XREF: MODEM_INT_HANDLER+48↓w
                                        ; RESET+1D↓w ...
                align 2
MODEM_STATUS_C4 db 0                    ; DATA XREF: MODEM_READ_STATUS:WAIT_READY↓r
                                        ; MODEM_READ_STATUS:WAIT_ACCEPT↓r ...
                align 2
MODEM_COMMAND_C6 db 0                   ; DATA XREF: MODEM_INT_HANDLER+42↓w
                                        ; MODEM_INT_HANDLER+4E↓w ...
                db 0F1h dup(0)
REG_INT_HANDLER dd 0FFFF0h              ; DATA XREF: COM2_REQ_INIT+1D↓o
                                        ; COM2_REQ_INIT+83↓r
UNKNOWN_FUNC_2  dd 0FFFF0h              ; DATA XREF: RESET+78↓r
UNKNOWN_FUNC_3  dd 0FFFF0h              ; DATA XREF: RESET+B↓r
DOS_REG_INT     dd 0FFFF0h              ; DATA XREF: MODEM_INT_HANDLER+C↓r
                                        ; RESET+6C↓r
DELAY_MS        dd 0                    ; DATA XREF: COM2_INPUT_IMPL+1F↓r
                                        ; GO_ON_HOOK+3↓r ...
                align 40h
dword_200       dd 0                    ; DATA XREF: GET_TIME_COUNTER+7↓r
                                        ; COM2_REQ_INIT+15↓r
                align 8
byte_208        db 8Eh dup(0)           ; DATA XREF: REG_INTERRUPT_82H+7↓o
COM2STAT_CMD_BUF_LEN dw 0               ; DATA XREF: COM2STAT_OUTPUT_IMPL+9↓r
                                        ; COM2STAT_OUTPUT_IMPL+F↓w ...
COM2STAT_CMD_BUF db 51h dup(0)          ; DATA XREF: COM2STAT_OUTPUT_IMPL+14↓o
                                        ; COM2STAT_OUTPUT_IMPL+1D↓o
AT_COMMAND_BUF  db 50h dup(0)           ; DATA XREF: AT_COMMAND_PARSER+6↓o
AT_COMMAND_BUF_LEN db 0                 ; DATA XREF: AT_COMMAND_PARSER+2↓r
                                        ; AT_COMMAND_PARSER+3E↓w ...
byte_33A        db 0                    ; DATA XREF: AT_COMMAND_PARSER+A6↓w
                                        ; PROCESS_INPUT+1A↓w ...
S_REG_NUMBER    db 0                    ; DATA XREF: PROCESS_INPUT_S+19↓w
                                        ; PROCESS_INPUT_S+2E↓r ...
a0              db 2,'0',0Dh            ; DATA XREF: TEST_FLAGS_DO_NUMBERS?+21↓o
                                        ; sub_178E+4↓o
aOk             db 6,0Dh,0Ah
                db 'OK',0Dh,0Ah
a1              db 2,'1',0Dh            ; DATA XREF: TEST_FLAGS_DO_NUMBERS?:loc_16AE↓o
aConnect        db 11,0Dh,0Ah
                db 'CONNECT',0Dh,0Ah
a2              db 2,'2',0Dh
aRing           db 8,0Dh,0Ah
                db 'RING',0Dh,0Ah
a3              db 2,'3',0Dh            ; DATA XREF: TEST_FLAGS_DO_NUMBERS?+2E↓o
aNoCarrier      db 14,0Dh,0Ah
                db 'NO CARRIER',0Dh,0Ah
a4              db 2,'4',0Dh            ; DATA XREF: TEST_FLAGS_DO_NUMBERS?+16↓o
aError          db 9,0Dh,0Ah
                db 'ERROR',0Dh,0Ah
a5              db 2,'5',0Dh            ; DATA XREF: TEST_FLAGS_DO_NUMBERS?+40↓o
aConnect1200    db 16,0Dh,0Ah
                db 'CONNECT 1200',0Dh,0Ah
a000            db 7,0Dh,0Ah            ; DATA XREF: seg000:11AD↓o sub_178E+27↓o ...
                db '000',0Dh,0Ah
REENT_FLAG?     db 0                    ; DATA XREF: COM2STAT_INT+1C↓r
                                        ; INT82H_HANDLER+1A↓r ...
                align 2
DevRequest_0    dd 0                    ; DATA XREF: COM2_STRATEGY↓w
                                        ; COM2STAT_INT+24↓r ...
COM2_INT_PROC_TABLE dw offset COM2_REQ_INIT ; DATA XREF: COM2_INT+1↓o
                dw offset COM2_REQ_NOTAPP
                dw offset COM2_REQ_NOTAPP
                dw offset INT_UNSUPPORTED_COMMAND
                dw offset COM2_REQ_INPUT
                dw offset COM2_REQ_NDESTRUCTIVE_INPUT
                dw offset COM2_REQ_INPUT_STATUS
                dw offset COM2_REQ_INPUT_FLUSH
                dw offset COM2_REQ_OUTPUT
                dw offset COM2_REQ_OUTPUT
                dw offset COM2_REQ_OUTPUT_FLUSH
                dw offset COM2_REQ_NOTAPP
COM2STAT_INT_PROC_TABLE dw offset COM2_REQ_NOTAPP ; DATA XREF: COM2STAT_INT+1↓o
                dw offset COM2_REQ_NOTAPP
                dw offset COM2_REQ_NOTAPP
                dw offset INT_UNSUPPORTED_COMMAND
                dw offset COM2STAT_REQ_INPUT
                dw offset COM2_REQ_NOTAPP
                dw offset COM2_REQ_NOTAPP
                dw offset COM2_REQ_NOTAPP
                dw offset COM2STAT_REQ_OUTPUT
                dw offset COM2_REQ_NOTAPP
                dw offset COM2_REQ_NOTAPP
                dw offset COM2_REQ_NOTAPP
MODEM_ACTIONS   dw offset RESET_MODEM   ; DATA XREF: DISPATCH_MODEM_ACTION+14↓o
                dw offset SET_BREAK_AND_CONNECT_TIMEOUT
                dw offset MODEM_DISABLE_CARRIER_UNCOND
                dw offset START_HANDSHAKE_AND_WAIT_CONNECT
                dw offset GO_OFF_HOOK
                dw offset GO_ON_HOOK
                dw offset SET_HANDSHAKE_MODE
                dw offset DIAL_NUMBER
                dw offset MODEM_DISABLE_CARRIER
                dw offset MODEM_ENABLE_CARRIER
                dw offset MODEM_SET_HANGUP_TIMERS
MODEM_SET_VOLUME_FROM_MEM_INDIR dw offset MODEM_SET_VOLUME_FROM_MEM
                                        ; DATA XREF: seg000:10B7↓o
                                        ; seg000:loc_11ED↓o
S_REG_WRITERS_TABLE dw 0                ; DATA XREF: PROCESS_INPUT_S+38↓o
                dw offset S1_WRITE
                dw offset S2_WRITE
                dw offset S3_WRITE
                dw offset S4_WRITE
                dw offset S5_WRITE
                dw offset S6_WRITE
                dw offset S7_WRITE
                dw offset S8_WRITE
                dw offset S9_WRITE
                dw offset S10_WRITE
                dw offset S11_WRITE
                dw offset S12_WRITE
S_REG_READERS_TABLE dw 0                ; DATA XREF: PROCESS_INPUT_S+41↓o
                dw offset S1_READ
                dw offset S2_READ
                dw offset S3_READ
                dw offset S4_READ
                dw offset S5_READ
                dw offset S6_READ
                dw offset S7_READ
                dw offset S8_READ
                dw offset S9_READ
                dw offset S10_READ
                dw offset S11_READ
                dw offset S12_READ
                dw offset loc_10E8
                dw offset loc_1128
                dw offset loc_1163

; =============== S U B R O U T I N E =======================================


COM2_STRATEGY   proc far                ; DATA XREF: seg000:Strategy_0↑o
                                        ; seg000:Strategy_1↑o
                mov     word ptr cs:DevRequest_0, bx ; ES:BX -> Device Request Block
                mov     word ptr cs:DevRequest_0+2, es
                retf
COM2_STRATEGY   endp


; =============== S U B R O U T I N E =======================================


COM2_INT        proc far                ; DATA XREF: seg000:Interrupt_0↑o
                push    si              ; Device Request Block:
                                        ; 0 db length
                                        ; 1 db unit number
                                        ; 2 db command code
                                        ; 5 d? reserved
                                        ; 0D d? command specific data
                mov     si, offset COM2_INT_PROC_TABLE
                jmp     short INT_HANDLER
COM2_INT        endp


; =============== S U B R O U T I N E =======================================


COM2STAT_INT    proc far                ; DATA XREF: seg000:Interrupt_1↑o
                push    si              ; Device Request Block:
                                        ; 0 db length
                                        ; 1 db unit number
                                        ; 2 db command code
                                        ; 5 d? reserved
                                        ; 0D d? command specific data
                mov     si, offset COM2STAT_INT_PROC_TABLE

INT_HANDLER:                            ; CODE XREF: COM2_INT+4↑j
                push    ax
                push    bx
                push    cx
                push    dx
                push    di
                push    bp
                push    ds
                push    es
                pushf
                mov     ax, ss
                mov     bx, sp
                cli
                mov     sp, 294h
                mov     cx, cs
                mov     ss, cx
                sti
                push    bx
                push    ax
                mov     al, cs:REENT_FLAG?
                cmp     al, 0
                jnz     short loc_478
                lds     bx, cs:DevRequest_0
                mov     cx, [bx+12h]
                mov     dx, [bx+14h]
                mov     al, [bx+2]
                cmp     al, 11
                ja      short INT_UNSUPPORTED_COMMAND
                xor     ah, ah
                shl     ax, 1
                add     si, ax
                les     di, [bx+0Eh]
                push    cs
                pop     ds
                assume ds:seg000
                jmp     word ptr [si]
; ---------------------------------------------------------------------------

loc_478:                                ; CODE XREF: COM2STAT_INT+22↑j
                mov     al, 0
                jmp     short COM2_REQ_NOTAPP
; ---------------------------------------------------------------------------

loc_47C:                                ; CODE XREF: COM2_REQ_NDESTRUCTIVE_INPUT+28↓j
                                        ; COM2_REQ_INPUT_STATUS:loc_4D4↓j ...
                mov     ah, 3
                jmp     short INT_EPILOGUE
; ---------------------------------------------------------------------------

INT_UNSUPPORTED_COMMAND:                ; CODE XREF: COM2STAT_INT+34↑j
                                        ; DATA XREF: seg000:03A8↑o ...
                mov     al, 3

INT_RET_ERROR:                          ; CODE XREF: COM2_REQ_INPUT:loc_509↓j
                                        ; COM2_REQ_OUTPUT:loc_56C↓j ...
                mov     ah, 10000001b
                jmp     short INT_EPILOGUE
; ---------------------------------------------------------------------------

COM2_REQ_NOTAPP:                        ; CODE XREF: COM2STAT_INT+45↑j
                                        ; COM2_REQ_NDESTRUCTIVE_INPUT+25↓j ...
                mov     ah, 1

INT_EPILOGUE:                           ; CODE XREF: COM2STAT_INT+49↑j
                                        ; COM2STAT_INT+4F↑j
                lds     bx, cs:DevRequest_0
                assume ds:nothing
                mov     [bx+3], ax
                pop     ax
                pop     bx
                cli
                mov     ss, ax
                mov     sp, bx
                sti
                popf
                pop     es
                pop     ds
                pop     bp
                pop     di
                pop     dx
                pop     cx
                pop     bx
                pop     ax
                pop     si
                retf
COM2STAT_INT    endp


; =============== S U B R O U T I N E =======================================


COM2_REQ_NDESTRUCTIVE_INPUT proc far    ; DATA XREF: seg000:03AC↑o
                cli
                mov     si, ds:RXBUF_READ_CURSOR
                cmp     si, ds:RXBUF_WRITE_CURSOR
                jz      short loc_4CA
                inc     si
                cmp     si, ds:RXBUF_SIZE
                jnz     short loc_4B8
                mov     si, 0

loc_4B8:                                ; CODE XREF: COM2_REQ_NDESTRUCTIVE_INPUT+10↑j
                les     bx, dword ptr ds:RXBUF_FARPTR_OFFSET
                mov     al, es:[bx+si]
                sti
                les     di, ds:DevRequest_0
                mov     es:[di+0Dh], al
                jmp     short COM2_REQ_NOTAPP
; ---------------------------------------------------------------------------

loc_4CA:                                ; CODE XREF: COM2_REQ_NDESTRUCTIVE_INPUT+9↑j
                sti
                jmp     short loc_47C
COM2_REQ_NDESTRUCTIVE_INPUT endp


; =============== S U B R O U T I N E =======================================


COM2_REQ_INPUT_STATUS proc far          ; DATA XREF: seg000:03AE↑o
                call    IS_RX_BUFFER_EMPTY
                jb      short loc_4D4
                jmp     short COM2_REQ_NOTAPP
; ---------------------------------------------------------------------------

loc_4D4:                                ; CODE XREF: COM2_REQ_INPUT_STATUS+3↑j
                jmp     short loc_47C
COM2_REQ_INPUT_STATUS endp


; =============== S U B R O U T I N E =======================================


IS_RX_BUFFER_EMPTY proc near            ; CODE XREF: COM2_REQ_INPUT_STATUS↑p
                                        ; INT82H_OP_READ_STATUS+3↓p
                cli
                mov     si, ds:RXBUF_READ_CURSOR
                cmp     si, ds:RXBUF_WRITE_CURSOR
                stc
                jz      short EXIT
                clc

EXIT:                                   ; CODE XREF: IS_RX_BUFFER_EMPTY+A↑j
                sti
                retn
IS_RX_BUFFER_EMPTY endp

; ---------------------------------------------------------------------------

COM2_REQ_OUTPUT_FLUSH:                  ; DATA XREF: seg000:03B6↑o
                call    IS_STATE_100h_FLAG
                jb      short loc_4EC
                jmp     short COM2_REQ_NOTAPP
; ---------------------------------------------------------------------------

loc_4EC:                                ; CODE XREF: seg000:04E8↑j
                jmp     short loc_47C

; =============== S U B R O U T I N E =======================================


IS_STATE_100h_FLAG proc near            ; CODE XREF: seg000:COM2_REQ_OUTPUT_FLUSH↑p
                                        ; INT82H_OP_READ_STATUS+D↓p
                mov     di, offset MODEM_STATE_BITS
                test    word ptr [di], MDM_UNKNOWN_100
                stc
                jz      short EXIT
                clc

EXIT:                                   ; CODE XREF: IS_STATE_100h_FLAG+8↑j
                retn
IS_STATE_100h_FLAG endp


; =============== S U B R O U T I N E =======================================


COM2_REQ_INPUT  proc far                ; DATA XREF: seg000:03AA↑o
                call    COM2_INPUT_IMPL
                lds     si, ds:DevRequest_0
                mov     [si+12h], cx
                jb      short loc_509
                jmp     COM2_REQ_NOTAPP
; ---------------------------------------------------------------------------

loc_509:                                ; CODE XREF: COM2_REQ_INPUT+A↑j
                jmp     INT_RET_ERROR
COM2_REQ_INPUT  endp


; =============== S U B R O U T I N E =======================================


COM2_INPUT_IMPL proc near               ; CODE XREF: COM2_REQ_INPUT↑p
                                        ; INT82H_OP_READ↓p
                push    cx

loc_50D:                                ; CODE XREF: COM2_INPUT_IMPL+42↓j
                mov     dx, ds:READ_CHAR_TIMEOUT
                mov     ax, 1
                cmp     dx, 0
                jnz     short loc_51C
                mov     dx, 1

loc_51C:                                ; CODE XREF: COM2_INPUT_IMPL+B↑j
                                        ; COM2_INPUT_IMPL+23↓j
                cli
                mov     si, ds:RXBUF_READ_CURSOR
                cmp     si, ds:RXBUF_WRITE_CURSOR
                jnz     short loc_531
                sti
                dec     dx
                jz      short loc_550
                call    ds:DELAY_MS
                jmp     short loc_51C
; ---------------------------------------------------------------------------

loc_531:                                ; CODE XREF: COM2_INPUT_IMPL+19↑j
                mov     dx, ds:RXBUF_SIZE
                dec     dx
                cmp     si, dx
                jnz     short loc_53F
                mov     si, 0
                jmp     short loc_540
; ---------------------------------------------------------------------------

loc_53F:                                ; CODE XREF: COM2_INPUT_IMPL+2C↑j
                inc     si

loc_540:                                ; CODE XREF: COM2_INPUT_IMPL+31↑j
                push    ds
                lds     bx, dword ptr ds:RXBUF_FARPTR_OFFSET
                mov     al, [bx+si]
                pop     ds
                mov     ds:RXBUF_READ_CURSOR, si
                sti
                stosb
                loop    loc_50D

loc_550:                                ; CODE XREF: COM2_INPUT_IMPL+1D↑j
                pop     ax
                sub     ax, cx
                mov     cx, ax
                clc
                retn
COM2_INPUT_IMPL endp

; ---------------------------------------------------------------------------

COM2_REQ_INPUT_FLUSH:                   ; DATA XREF: seg000:03B0↑o
                call    INT82H_OP_BUFFER_FLUSH
                jmp     COM2_REQ_NOTAPP

; =============== S U B R O U T I N E =======================================


COM2_REQ_OUTPUT proc far                ; DATA XREF: seg000:03B2↑o seg000:03B4↑o
                call    COM2_OUTPUT_IMPL
                lds     si, ds:DevRequest_0
                mov     [si+12h], cx
                jb      short loc_56C
                jmp     COM2_REQ_NOTAPP
; ---------------------------------------------------------------------------

loc_56C:                                ; CODE XREF: COM2_REQ_OUTPUT+A↑j
                jmp     INT_RET_ERROR
COM2_REQ_OUTPUT endp


; =============== S U B R O U T I N E =======================================


COM2_OUTPUT_IMPL proc near              ; CODE XREF: COM2_REQ_OUTPUT↑p
                                        ; INT82H_OP_WRITE↓p
                push    cx
                mov     si, di
                mov     di, offset STATE_BITS
                test    word ptr [di], 8000h
                jnz     short COMMAND_MODE
                call    ESCAPE_GUARD_WHAT??
                jnb     short loc_59E
                test    word ptr [di], 4000h
                jnz     short FALLBACK_TO_COMMAND_MODE
                call    DETECT_HAYES_ESCAPE_SEQUENCE
                jnb     short loc_59E
                or      word ptr [di], 4000h
                jmp     short DONE
; ---------------------------------------------------------------------------

FALLBACK_TO_COMMAND_MODE:               ; CODE XREF: COM2_OUTPUT_IMPL+15↑j
                or      word ptr [di], 8000h
                and     word ptr [di], 0BFFFh

COMMAND_MODE:                           ; CODE XREF: COM2_OUTPUT_IMPL+A↑j
                call    AT_COMMAND_PARSER
                jmp     short DONE
; ---------------------------------------------------------------------------

loc_59E:                                ; CODE XREF: COM2_OUTPUT_IMPL+F↑j
                                        ; COM2_OUTPUT_IMPL+1A↑j
                mov     di, offset MODEM_STATE_BITS
                test    ds:STATE_BITS, 2000h
                jnz     short loc_5C9

SEND_BYTES_LOOP:                        ; CODE XREF: COM2_OUTPUT_IMPL+7D↓j
                test    ds:STATE_BITS, 2000h
                jnz     short loc_5C9
                test    word ptr [di], 8000h
                jz      short loc_5F8
                mov     dx, 8000h

loc_5BA:                                ; CODE XREF: COM2_OUTPUT_IMPL+58↓j
                mov     al, 2
                call    MODEM_READ_STATUS_CRIT
                test    ah, 2
                jnz     short loc_5C9
                dec     dx
                jz      short loc_5F8
                jmp     short loc_5BA
; ---------------------------------------------------------------------------

loc_5C9:                                ; CODE XREF: COM2_OUTPUT_IMPL+38↑j
                                        ; COM2_OUTPUT_IMPL+40↑j ...
                lods    byte ptr es:[si]
                test    ds:HAYES_STATE, 80h
                jz      short SEND_BYTE
                push    ax
                push    ds
                push    cx
                dec     si
                mov     ax, es
                mov     ds, ax
                mov     cx, 1
                call    RX_BUFFER_WRITE_BYTES
                inc     si
                pop     cx
                pop     ds
                pop     ax

SEND_BYTE:                              ; CODE XREF: COM2_OUTPUT_IMPL+62↑j
                mov     ah, al
                mov     al, 0
                call    MODEM_WRITE_REG_CRIT
                loop    SEND_BYTES_LOOP

DONE:                                   ; CODE XREF: COM2_OUTPUT_IMPL+20↑j
                                        ; COM2_OUTPUT_IMPL+2D↑j
                call    UPDATE_LAST_COM2_OUTPUT_TIMESTAMP
                pop     ax
                sub     ax, cx
                mov     cx, ax
                clc
                retn
; ---------------------------------------------------------------------------

loc_5F8:                                ; CODE XREF: COM2_OUTPUT_IMPL+46↑j
                                        ; COM2_OUTPUT_IMPL+56↑j
                pop     ax
                sub     ax, cx
                mov     cx, ax
                mov     al, 2
                stc
                retn
COM2_OUTPUT_IMPL endp

; ---------------------------------------------------------------------------

COM2STAT_REQ_INPUT:                     ; DATA XREF: seg000:03C2↑o
                call    COM2STAT_INPUT_IMPL
                lds     si, ds:DevRequest_0
                mov     [si+12h], cx
                jmp     COM2_REQ_NOTAPP

; =============== S U B R O U T I N E =======================================


COM2STAT_INPUT_IMPL proc near           ; CODE XREF: seg000:COM2STAT_REQ_INPUT↑p
                                        ; INT82H_OP_READ_QUERY↓p
                mov     si, 68h ; 'h'
                mov     bl, ds:byte_6D
                xor     bh, bh
                cmp     cx, 5
                jbe     short loc_61F
                mov     cx, 5

loc_61F:                                ; CODE XREF: COM2STAT_INPUT_IMPL+C↑j
                push    cx

loc_620:                                ; CODE XREF: COM2STAT_INPUT_IMPL+16↓j
                mov     al, [bx+si]
                inc     bx
                stosb
                loop    loc_620
                cmp     bx, 5
                jb      short loc_62E
                mov     bx, 0

loc_62E:                                ; CODE XREF: COM2STAT_INPUT_IMPL+1B↑j
                mov     ds:byte_6D, bl
                pop     cx
                clc
                retn
COM2STAT_INPUT_IMPL endp

; ---------------------------------------------------------------------------

COM2STAT_REQ_OUTPUT:                    ; DATA XREF: seg000:03CA↑o
                call    COM2STAT_OUTPUT_IMPL
                lds     si, ds:DevRequest_0
                mov     [si+12h], cx
                jb      short loc_644
                jmp     COM2_REQ_NOTAPP
; ---------------------------------------------------------------------------

loc_644:                                ; CODE XREF: seg000:063F↑j
                jmp     INT_RET_ERROR

; =============== S U B R O U T I N E =======================================


COM2STAT_OUTPUT_IMPL proc near          ; CODE XREF: seg000:COM2STAT_REQ_OUTPUT↑p
                                        ; INT82H_OP_WRITE_COMMAND↓p
                push    cx
                mov     si, di

READ_UNTIL_CARRIAGE_RETURN:             ; CODE XREF: COM2STAT_OUTPUT_IMPL+2F↓j
                lods    byte ptr es:[si]
                cmp     al, 0Dh
                jnz     short NOT_CARRIAGE_RETURN
                mov     cx, ds:COM2STAT_CMD_BUF_LEN
                xor     ax, ax
                mov     ds:COM2STAT_CMD_BUF_LEN, ax
                push    cs
                pop     es
                assume es:seg000
                mov     si, offset COM2STAT_CMD_BUF
                jmp     short PARSING_LOOP
; ---------------------------------------------------------------------------

NOT_CARRIAGE_RETURN:                    ; CODE XREF: COM2STAT_OUTPUT_IMPL+7↑j
                mov     di, ds:COM2STAT_CMD_BUF_LEN
                mov     bx, offset COM2STAT_CMD_BUF
                mov     [bx+di], al
                inc     di
                cmp     di, 81
                jb      short BUFFER_NOT_FULL
                jmp     BAD_INPUT
; ---------------------------------------------------------------------------

BUFFER_NOT_FULL:                        ; CODE XREF: COM2STAT_OUTPUT_IMPL+26↑j
                mov     ds:COM2STAT_CMD_BUF_LEN, di
                loop    READ_UNTIL_CARRIAGE_RETURN
                xor     ax, ax
                jmp     SUCCESS
; ---------------------------------------------------------------------------

PARSING_LOOP:                           ; CODE XREF: COM2STAT_OUTPUT_IMPL+17↑j
                                        ; COM2STAT_OUTPUT_IMPL+AC↓j ...
                cmp     cx, 0
                jnz     short START_PARSING
                jmp     SUCCESS
; ---------------------------------------------------------------------------

START_PARSING:                          ; CODE XREF: COM2STAT_OUTPUT_IMPL+39↑j
                lods    byte ptr es:[si]
                dec     cx
                call    CONVERT_CHAR_TO_UPPERCASE
                cmp     al, 'B'
                jnz     short CHECK_P
                mov     di, offset COM2STAT_PARSE_BAUD_RATE
                jmp     short DO
; ---------------------------------------------------------------------------

CHECK_P:                                ; CODE XREF: COM2STAT_OUTPUT_IMPL+46↑j
                cmp     al, 'P'
                jnz     short CHECK_S
                mov     di, offset COM2STAT_PARSE_PARITY
                jmp     short DO
; ---------------------------------------------------------------------------

CHECK_S:                                ; CODE XREF: COM2STAT_OUTPUT_IMPL+4F↑j
                cmp     al, 'S'
                jnz     short CHECK_Q
                mov     di, offset COM2STAT_PARSE_STOP_BITS
                jmp     short DO
; ---------------------------------------------------------------------------

CHECK_Q:                                ; CODE XREF: COM2STAT_OUTPUT_IMPL+58↑j
                cmp     al, 'Q'
                jnz     short CHECK_F
                mov     di, offset COM2STAT_BUILD_STATUS_REPLY
                jmp     short DO
; ---------------------------------------------------------------------------

CHECK_F:                                ; CODE XREF: COM2STAT_OUTPUT_IMPL+61↑j
                cmp     al, 'F'
                jnz     short CHECK_DA
                mov     di, offset INT82H_OP_BUFFER_FLUSH
                jmp     short DO
; ---------------------------------------------------------------------------

CHECK_DA:                               ; CODE XREF: COM2STAT_OUTPUT_IMPL+6A↑j
                cmp     al, 'D'
                jnz     short CHECK_C
                jcxz    short BAD_INPUT
                lods    byte ptr es:[si]
                dec     cx
                call    CONVERT_CHAR_TO_UPPERCASE
                cmp     al, 'A'
                jnz     short CHECK_B
                mov     di, offset COM2STAT_PARSE_DATA_BITS
                jmp     short DO
; ---------------------------------------------------------------------------

CHECK_B:                                ; CODE XREF: COM2STAT_OUTPUT_IMPL+7F↑j
                cmp     al, 'B'
                jnz     short BAD_INPUT
                mov     di, offset COM2STAT_PARSE_HIGH_SPEED_DELAY
                jmp     short DO
; ---------------------------------------------------------------------------

CHECK_C:                                ; CODE XREF: COM2STAT_OUTPUT_IMPL+73↑j
                cmp     al, 'C'
                jnz     short CHECK_I
                mov     di, offset COM2STAT_PARSE_READ_TIMEOUT
                jmp     short DO
; ---------------------------------------------------------------------------

CHECK_I:                                ; CODE XREF: COM2STAT_OUTPUT_IMPL+91↑j
                cmp     al, 'I'
                jnz     short CHECK_A
                mov     di, offset RESET
                jmp     short DO
; ---------------------------------------------------------------------------

CHECK_A:                                ; CODE XREF: COM2STAT_OUTPUT_IMPL+9A↑j
                cmp     al, 'A'
                jnz     short CHECK_V
                mov     di, offset COM2STAT_PARSE_RX_BUF
                jmp     short DO
; ---------------------------------------------------------------------------

CHECK_V:                                ; CODE XREF: COM2STAT_OUTPUT_IMPL+A3↑j
                cmp     al, 'V'
                jnz     short PARSING_LOOP
                mov     di, offset COM2STAT_PARSE_VOLUME

DO:                                     ; CODE XREF: COM2STAT_OUTPUT_IMPL+4B↑j
                                        ; COM2STAT_OUTPUT_IMPL+54↑j ...
                call    di ; COM2STAT_PARSE_BAUD_RATE
                jb      short ERROR
                mov     al, ','         ; Process commands between commas
                xchg    si, di
                repne scasb
                xchg    si, di
                jnz     short SUCCESS
                jmp     PARSING_LOOP
; ---------------------------------------------------------------------------

BAD_INPUT:                              ; CODE XREF: COM2STAT_OUTPUT_IMPL+28↑j
                                        ; COM2STAT_OUTPUT_IMPL+75↑j ...
                xor     ah, ah
                mov     al, 3

ERROR:                                  ; CODE XREF: COM2STAT_OUTPUT_IMPL+B3↑j
                pop     cx
                stc
                retn
; ---------------------------------------------------------------------------

SUCCESS:                                ; CODE XREF: COM2STAT_OUTPUT_IMPL+33↑j
                                        ; COM2STAT_OUTPUT_IMPL+3B↑j ...
                pop     ax
                sub     ax, cx
                mov     cx, ax
                clc
                retn
COM2STAT_OUTPUT_IMPL endp


; =============== S U B R O U T I N E =======================================


MODEM_INT_HANDLER proc far              ; DATA XREF: COM2_REQ_INIT+7F↓o
                push    ax
                push    ds
                push    bx
                push    es
                push    si
                push    di
                push    cs
                pop     ds
                assume ds:seg000
                mov     ax, 4
                push    ax
                call    DOS_REG_INT     ; Ack int
                mov     al, 2           ; Try to understand interrupt reason
                call    MODEM_READ_STATUS
                mov     bl, ah
                mov     di, offset MODEM_STATE_BITS
                cmp     bl, 0
                jz      short LINE_STATE_CHANGED
                test    bl, 80h
                jz      short BYTE_FOR_READ_APPEARED

LINE_STATE_CHANGED:                     ; CODE XREF: MODEM_INT_HANDLER+1D↑j
                mov     al, 1
                call    MODEM_READ_STATUS
                test    ah, 80h
                jz      short CHECK_LINE_STATE
                cmp     SHADOW_MODEM_OPTIONS.UNK_8, 0FFh
                jz      short FORCE_COMMAND_MODE_AND_CHECK_MODEM_STATUS

RESET_PATH:                             ; Reset state
                and     word ptr [di], not MDM_ONLINE
                and     word ptr [di], 1010111000001111b
                mov     ax, 0DFE0h
                mov     es, ax
                assume es:nothing
                mov     es:MODEM_COMMAND_C6, 0C0h ; Reset modem
                mov     es:MODEM_CONTROL_C2, 7Fh
                mov     es:MODEM_COMMAND_C6, 3
                mov     si, offset SHADOW_MODEM_OPTIONS.BAUD_RATE_AND_DELAY
                mov     al, 1
                mov     cx, 8

WRITE_OPTIONS_TO_MODEM_LOOP:            ; CODE XREF: MODEM_INT_HANDLER+6B↓j
                mov     ah, [si]
                cmp     al, 1
                jnz     short WRITE_OPTION_TO_MODEM
                and     ah, 111111b

WRITE_OPTION_TO_MODEM:                  ; CODE XREF: MODEM_INT_HANDLER+60↑j
                call    MODEM_WRITE_REG
                inc     al
                inc     si
                loop    WRITE_OPTIONS_TO_MODEM_LOOP

FORCE_COMMAND_MODE_AND_CHECK_MODEM_STATUS: ; CODE XREF: MODEM_INT_HANDLER+33↑j
                or      STATE_BITS, 8000h
                mov     al, 1
                call    MODEM_READ_STATUS

CHECK_LINE_STATE:                       ; CODE XREF: MODEM_INT_HANDLER+2C↑j
                test    ah, 10h
                jz      short RESET_CARRIER_DETECTED_FLAG
                or      word ptr [di], MDM_CARRIER_DETECTED
                jmp     short CHECK_RING
; ---------------------------------------------------------------------------

RESET_CARRIER_DETECTED_FLAG:            ; CODE XREF: MODEM_INT_HANDLER+7B↑j
                and     word ptr [di], not MDM_CARRIER_DETECTED

CHECK_RING:                             ; CODE XREF: MODEM_INT_HANDLER+81↑j
                test    ah, 8
                jz      short CHECK_LINE_READINESS
                or      word ptr [di], MDM_RING_SEEN
                inc     REG_S1

CHECK_LINE_READINESS:                   ; CODE XREF: MODEM_INT_HANDLER+8A↑j
                test    ah, 4
                jz      short LINE_NOT_READY
                or      word ptr [di], MDM_LINE_READY
                jmp     short BYTE_FOR_READ_APPEARED
; ---------------------------------------------------------------------------

LINE_NOT_READY:                         ; CODE XREF: MODEM_INT_HANDLER+97↑j
                and     word ptr [di], not MDM_LINE_READY

BYTE_FOR_READ_APPEARED:                 ; CODE XREF: MODEM_INT_HANDLER+22↑j
                                        ; MODEM_INT_HANDLER+9D↑j
                test    word ptr [di], MDM_ONLINE
                jz      short DONE      ; Notify modem that interrupt handled
                test    bl, 4
                jz      short DONE      ; Notify modem that interrupt handled
                mov     si, RXBUF_WRITE_CURSOR
                mov     ax, RXBUF_READ_CURSOR
                dec     ax
                cmp     si, ax
                jz      short READ_DATA_BYTE
                mov     ax, RXBUF_SIZE
                dec     ax
                cmp     si, ax
                jnz     short loc_7E5
                cmp     RXBUF_READ_CURSOR, 0
                jz      short READ_DATA_BYTE
                mov     si, 0
                jmp     short READ_DATA_BYTE
; ---------------------------------------------------------------------------

loc_7E5:                                ; CODE XREF: MODEM_INT_HANDLER+C0↑j
                inc     si

READ_DATA_BYTE:                         ; CODE XREF: MODEM_INT_HANDLER+B8↑j
                                        ; MODEM_INT_HANDLER+C7↑j ...
                mov     al, 0
                call    MODEM_READ_STATUS
                push    bx
                push    ds
                lds     bx, dword ptr cs:RXBUF_FARPTR_OFFSET
                assume ds:nothing
                mov     [bx+si], ah
                pop     ds
                pop     bx
                mov     ds:RXBUF_WRITE_CURSOR, si
                test    bl, 40h
                jz      short CHECK_20h???
                or      word ptr [di], MDM_UNKNOWN_800

CHECK_20h???:                           ; CODE XREF: MODEM_INT_HANDLER+E6↑j
                test    bl, 20h
                jz      short CHECK_10h???
                or      word ptr [di], MDM_UNKNOWN_400

CHECK_10h???:                           ; CODE XREF: MODEM_INT_HANDLER+EF↑j
                test    bl, 10h
                jz      short CHECK_2h???
                or      word ptr [di], MDM_UNKNOWN_200

CHECK_2h???:                            ; CODE XREF: MODEM_INT_HANDLER+F8↑j
                test    bl, 2
                jz      short loc_820
                or      word ptr [di], MDM_UNKNOWN_100
                jmp     short DONE      ; Notify modem that interrupt handled
; ---------------------------------------------------------------------------

loc_820:                                ; CODE XREF: MODEM_INT_HANDLER+101↑j
                and     word ptr [di], not MDM_UNKNOWN_100

DONE:                                   ; CODE XREF: MODEM_INT_HANDLER+A7↑j
                                        ; MODEM_INT_HANDLER+AC↑j ...
                mov     al, 5           ; Notify modem that interrupt handled
                call    MODEM_EXEC_COMMAND
                pop     di
                pop     si
                pop     es
                assume es:nothing
                pop     bx
                pop     ds
                pop     ax
                iret
MODEM_INT_HANDLER endp


; =============== S U B R O U T I N E =======================================


COM2STAT_PARSE_BAUD_RATE proc near      ; CODE XREF: COM2STAT_OUTPUT_IMPL:DO↑p
                                        ; DATA XREF: COM2STAT_OUTPUT_IMPL+48↑o
                call    SEARCH_EQUALS_SIGN
                jnz     short ERROR
                jcxz    short ERROR
                lods    byte ptr es:[si]
                dec     cx
                cmp     al, '1'
                jnz     short NOT_1
                mov     al, 7
                jmp     short SET_BAUD_RATE
; ---------------------------------------------------------------------------

NOT_1:                                  ; CODE XREF: COM2STAT_PARSE_BAUD_RATE+C↑j
                cmp     al, '3'
                jnz     short ERROR
                mov     al, 5

SET_BAUD_RATE:                          ; CODE XREF: COM2STAT_PARSE_BAUD_RATE+10↑j
                call    INT82H_OP_SET_BAUD_RATE
                retn
; ---------------------------------------------------------------------------

ERROR:                                  ; CODE XREF: COM2STAT_PARSE_BAUD_RATE+3↑j
                                        ; COM2STAT_PARSE_BAUD_RATE+5↑j ...
                xor     ah, ah
                mov     al, 3
                stc
                retn
COM2STAT_PARSE_BAUD_RATE endp


; =============== S U B R O U T I N E =======================================


INT82H_OP_SET_BAUD_RATE proc near       ; CODE XREF: COM2STAT_PARSE_BAUD_RATE:SET_BAUD_RATE↑p
                                        ; INT82H_HANDLER+38↓p
                                        ; DATA XREF: ...
                mov     ah, ds:SHADOW_MODEM_OPTIONS.BAUD_RATE_AND_DELAY
                cmp     al, 7           ; 1200 bps
                jnz     short IS_300_BOD
                and     ah, 11111011b
                jmp     short SET_BAUD_RATE
; ---------------------------------------------------------------------------

IS_300_BOD:                             ; CODE XREF: INT82H_OP_SET_BAUD_RATE+6↑j
                cmp     al, 5           ; 300 bps
                jnz     short ERROR
                or      ah, 00000100b

SET_BAUD_RATE:                          ; CODE XREF: INT82H_OP_SET_BAUD_RATE+B↑j
                mov     al, 1
                call    MODEM_WRITE_REG_CRIT
                retn
; ---------------------------------------------------------------------------

ERROR:                                  ; CODE XREF: INT82H_OP_SET_BAUD_RATE+F↑j
                xor     ah, ah
                mov     al, 3
                stc
                retn
INT82H_OP_SET_BAUD_RATE endp


; =============== S U B R O U T I N E =======================================


COM2STAT_PARSE_READ_TIMEOUT proc near   ; DATA XREF: COM2STAT_OUTPUT_IMPL+93↑o
                call    SEARCH_EQUALS_SIGN
                call    PROCESS_S_REG
                jb      short BAD_INPUT
                mov     dx, ax
                call    INT82H_OP_SET_READ_CHAR_TIMEOUT
                jmp     short DONE
; ---------------------------------------------------------------------------

BAD_INPUT:                              ; CODE XREF: COM2STAT_PARSE_READ_TIMEOUT+6↑j
                xor     ah, ah
                mov     al, 3
                stc

DONE:                                   ; CODE XREF: COM2STAT_PARSE_READ_TIMEOUT+D↑j
                retn
COM2STAT_PARSE_READ_TIMEOUT endp


; =============== S U B R O U T I N E =======================================


INT82H_OP_SET_READ_CHAR_TIMEOUT proc near
                                        ; CODE XREF: COM2STAT_PARSE_READ_TIMEOUT+A↑p
                                        ; INT82H_HANDLER+38↓p
                                        ; DATA XREF: ...
                cli
                mov     ds:READ_CHAR_TIMEOUT, dx
                sti
                clc
                retn
INT82H_OP_SET_READ_CHAR_TIMEOUT endp


; =============== S U B R O U T I N E =======================================


COM2STAT_PARSE_DATA_BITS proc near      ; DATA XREF: COM2STAT_OUTPUT_IMPL+81↑o
                call    SEARCH_EQUALS_SIGN
                jnz     short BAD_INPUT
                jcxz    short BAD_INPUT
                lods    byte ptr es:[si]
                dec     cx
                sub     al, '0'
                call    INT82H_OP_SET_DATA_BITS
                retn
; ---------------------------------------------------------------------------

BAD_INPUT:                              ; CODE XREF: COM2STAT_PARSE_DATA_BITS+3↑j
                                        ; COM2STAT_PARSE_DATA_BITS+5↑j
                xor     ah, ah
                mov     al, 3
                stc
                retn
COM2STAT_PARSE_DATA_BITS endp


; =============== S U B R O U T I N E =======================================


INT82H_OP_SET_DATA_BITS proc near       ; CODE XREF: COM2STAT_PARSE_DATA_BITS+C↑p
                                        ; INT82H_HANDLER+38↓p
                                        ; DATA XREF: ...
                cmp     al, 5
                jnz     short CHECK_6
                mov     ah, 0
                jmp     short SET_DATA_BITS
; ---------------------------------------------------------------------------

CHECK_6:                                ; CODE XREF: INT82H_OP_SET_DATA_BITS+2↑j
                cmp     al, 6
                jnz     short CHECK_7
                mov     ah, 1
                jmp     short SET_DATA_BITS
; ---------------------------------------------------------------------------

CHECK_7:                                ; CODE XREF: INT82H_OP_SET_DATA_BITS+A↑j
                cmp     al, 7
                jnz     short CHECK_8
                mov     ah, 2
                jmp     short SET_DATA_BITS
; ---------------------------------------------------------------------------

CHECK_8:                                ; CODE XREF: INT82H_OP_SET_DATA_BITS+12↑j
                cmp     al, 8
                jnz     short BAD_INPUT
                mov     ah, 3

SET_DATA_BITS:                          ; CODE XREF: INT82H_OP_SET_DATA_BITS+6↑j
                                        ; INT82H_OP_SET_DATA_BITS+E↑j ...
                mov     bh, ds:SHADOW_MODEM_OPTIONS.BITS_AND_PARITY ; Data bits, stop bits, parity
                and     bh, 11111100b
                or      ah, bh
                mov     al, 2
                call    MODEM_WRITE_REG_CRIT
                clc
                retn
; ---------------------------------------------------------------------------

BAD_INPUT:                              ; CODE XREF: INT82H_OP_SET_DATA_BITS+1A↑j
                xor     ah, ah
                mov     al, 3
                stc
                retn
INT82H_OP_SET_DATA_BITS endp


; =============== S U B R O U T I N E =======================================


COM2STAT_PARSE_HIGH_SPEED_DELAY proc near ; DATA XREF: COM2STAT_OUTPUT_IMPL+8A↑o

; FUNCTION CHUNK AT 08F9 SIZE 00000006 BYTES

                call    SEARCH_EQUALS_SIGN
                jnz     short INT82H_BAD_INPUT_2
                jcxz    short INT82H_BAD_INPUT_2
                mov     dl, es:[si+1]
                call    CONVERT_CHAR_TO_UPPERCASE
                mov     al, 0
                cmp     dl, 'F'
                jz      short loc_8F5
                mov     al, 0FFh
                cmp     dl, 'N'
                jnz     short INT82H_BAD_INPUT_2

loc_8F5:                                ; CODE XREF: COM2STAT_PARSE_HIGH_SPEED_DELAY+13↑j
                call    INT82H_OP_HIGH_SPEED_DELAY
                retn
COM2STAT_PARSE_HIGH_SPEED_DELAY endp

; ---------------------------------------------------------------------------
; START OF FUNCTION CHUNK FOR INT82H_OP_HIGH_SPEED_DELAY
;   ADDITIONAL PARENT FUNCTION COM2STAT_PARSE_HIGH_SPEED_DELAY

INT82H_BAD_INPUT_2:                     ; CODE XREF: COM2STAT_PARSE_HIGH_SPEED_DELAY+3↑j
                                        ; COM2STAT_PARSE_HIGH_SPEED_DELAY+5↑j ...
                xor     ah, ah
                mov     al, 3
                stc
                retn
; END OF FUNCTION CHUNK FOR INT82H_OP_HIGH_SPEED_DELAY

; =============== S U B R O U T I N E =======================================


INT82H_OP_HIGH_SPEED_DELAY proc near    ; CODE XREF: COM2STAT_PARSE_HIGH_SPEED_DELAY:loc_8F5↑p
                                        ; INT82H_HANDLER+38↓p
                                        ; DATA XREF: ...

; FUNCTION CHUNK AT 08F9 SIZE 00000006 BYTES

                mov     ah, ds:SHADOW_MODEM_OPTIONS.BAUD_RATE_AND_DELAY
                cmp     al, 0           ; Turn off
                jz      short TURN_OFF
                cmp     al, 0FFh        ; Turn on
                jnz     short INT82H_BAD_INPUT_2
                or      ah, 100000b
                jmp     short SET_HIGH_SPEED_DELAY
; ---------------------------------------------------------------------------

TURN_OFF:                               ; CODE XREF: INT82H_OP_HIGH_SPEED_DELAY+6↑j
                and     ah, 11011111b

SET_HIGH_SPEED_DELAY:                   ; CODE XREF: INT82H_OP_HIGH_SPEED_DELAY+F↑j
                mov     al, 1
                call    MODEM_WRITE_REG_CRIT
                clc
                retn
INT82H_OP_HIGH_SPEED_DELAY endp


; =============== S U B R O U T I N E =======================================


COM2STAT_PARSE_RX_BUF proc near         ; DATA XREF: COM2STAT_OUTPUT_IMPL+A5↑o
                jcxz    short USE_DEFAULT_BUFFER
                call    SEARCH_EQUALS_SIGN
                jnz     short USE_DEFAULT_BUFFER
                jcxz    short USE_DEFAULT_BUFFER
                push    cx
                push    es
                mov     di, es:[si]
                mov     ax, es:[si+2]
                mov     cx, es:[si+4]
                mov     es, ax
                jmp     short SET_BUFFER
; ---------------------------------------------------------------------------
                nop

USE_DEFAULT_BUFFER:                     ; CODE XREF: COM2STAT_PARSE_RX_BUF↑j
                                        ; COM2STAT_PARSE_RX_BUF+5↑j ...
                push    cx
                push    es
                mov     cx, 0

SET_BUFFER:                             ; CODE XREF: COM2STAT_PARSE_RX_BUF+18↑j
                call    INT82H_OP_SET_RX_BUFFER
                pop     es
                pop     cx
                retn
COM2STAT_PARSE_RX_BUF endp


; =============== S U B R O U T I N E =======================================


INT82H_OP_SET_RX_BUFFER proc near       ; CODE XREF: COM2STAT_PARSE_RX_BUF:SET_BUFFER↑p
                                        ; INT82H_HANDLER+38↓p
                                        ; DATA XREF: ...
                cmp     cx, 0
                jz      short USE_DEFAULT_BUFFER
                mov     ax, es
                cmp     ax, 0
                jnz     short USE_USER_BUFFER

USE_DEFAULT_BUFFER:                     ; CODE XREF: INT82H_OP_SET_RX_BUFFER+3↑j
                mov     di, 0B8h
                push    ds
                pop     es
                mov     cx, 256

USE_USER_BUFFER:                        ; CODE XREF: INT82H_OP_SET_RX_BUFFER+A↑j
                mov     ds:RXBUF_FARPTR_OFFSET, di
                mov     ds:RXBUF_FARPTR_SEG, es
                mov     ds:RXBUF_SIZE, cx
                call    INT82H_OP_BUFFER_FLUSH
                clc
                retn
INT82H_OP_SET_RX_BUFFER endp


; =============== S U B R O U T I N E =======================================


INT82H_OP_BUFFER_FLUSH proc near        ; CODE XREF: seg000:COM2_REQ_INPUT_FLUSH↑p
                                        ; INT82H_OP_SET_RX_BUFFER+20↑p ...
                cli
                xor     ax, ax
                mov     ds:RXBUF_WRITE_CURSOR, ax
                mov     ds:RXBUF_READ_CURSOR, ax
                sti
                clc
                retn
INT82H_OP_BUFFER_FLUSH endp


; =============== S U B R O U T I N E =======================================


RESET           proc near               ; CODE XREF: INT82H_OP_INIT↓p
                                        ; PROCESS_INPUT_Z↓p ...
                push    ax
                push    bx
                push    cx
                push    si
                push    di
                push    ds
                push    es
                mov     ax, 4
                push    ax
                call    cs:UNKNOWN_FUNC_3
                mov     ax, 0DFE0h      ; Reset modem
                mov     es, ax
                assume es:nothing
                mov     al, 0C0h
                mov     es:MODEM_COMMAND_C6, al
                mov     al, 7Fh
                mov     es:MODEM_CONTROL_C2, al
                mov     al, 3
                mov     es:MODEM_COMMAND_C6, al
                mov     al, 4Bh ; 'K'
                mov     ds:VOLUME_VALUE, al
                mov     al, 0
                call    MODEM_SET_VOLUME
                mov     ax, cs
                mov     es, ax
                assume es:seg000
                mov     ds, ax
                assume ds:seg000
                mov     si, offset DEFAULT_MODEM_OPTIONS
                mov     di, offset SHADOW_MODEM_OPTIONS
                mov     cx, 9
                rep movsb
                mov     si, offset SHADOW_MODEM_OPTIONS.BAUD_RATE_AND_DELAY
                mov     al, 1
                mov     cx, 8

loc_9BB:                                ; CODE XREF: RESET+52↓j
                mov     ah, [si]
                call    MODEM_WRITE_REG_CRIT
                inc     al
                inc     si
                loop    loc_9BB
                mov     ax, 0B8h
                mov     RXBUF_FARPTR_OFFSET, ax
                mov     ax, ds
                mov     RXBUF_FARPTR_SEG, ax
                mov     ax, 100h
                mov     RXBUF_SIZE, ax
                call    INT82H_OP_BUFFER_FLUSH
                mov     ax, 4
                push    ax
                call    DOS_REG_INT
                mov     ax, 4
                push    ax
                mov     ax, 0
                push    ax
                call    UNKNOWN_FUNC_2
                mov     ax, 30
                mov     REG_S7, ax      ; Seconds
                mov     ax, 2000
                mov     word_5A, ax
                mov     ax, 600
                mov     REG_S9, ax      ; Milliseconds
                mov     ax, 700
                mov     READ_CHAR_TIMEOUT, ax
                mov     REG_S10, ax     ; Time in milliseconds to wait before disconnecting after a remote hangs up.
                mov     ax, 1000h
                mov     HAYES_STATE, ax
                xor     ax, ax
                mov     REG_S1, ax
                mov     MODEM_STATE_BITS, ax
                mov     word_96, ax
                mov     byte_6E, al
                mov     ESCAPE_CHAR_COUNT, al
                mov     al, '+'
                mov     REG_S2, al
                mov     ax, 1000
                mov     REG_S12, ax     ; Milliseconds
                mov     ax, 8000h
                mov     STATE_BITS, ax
                mov     ax, 70
                mov     REG_S11, ax     ; Speed of the touch-tone dialer. In milliseconds.
                call    GET_TIME_COUNTER
                mov     word ptr LAST_COM2_OUTPUT_TIME, ax
                mov     word ptr LAST_COM2_OUTPUT_TIME+2, bx
                mov     al, 8
                mov     REG_S5, al
                mov     ah, 0Dh
                mov     REG_S3, ah      ; Value of the carriage return.
                mov     al, 0Ah
                mov     REG_S4, al
                call    sub_178E
                pop     es
                assume es:nothing
                pop     ds
                assume ds:nothing
                pop     di
                pop     si
                pop     cx
                pop     bx
                pop     ax
                clc
                retn
RESET           endp


; =============== S U B R O U T I N E =======================================


COM2STAT_PARSE_PARITY proc near         ; DATA XREF: COM2STAT_OUTPUT_IMPL+51↑o

; FUNCTION CHUNK AT 0A79 SIZE 00000006 BYTES
; FUNCTION CHUNK AT 0A7F SIZE 00000009 BYTES

                call    SEARCH_EQUALS_SIGN
                jnz     short BAD_INPUT_3
                jcxz    short BAD_INPUT_3
                lods    byte ptr es:[si]
                dec     cx
                call    CONVERT_CHAR_TO_UPPERCASE
                push    si
                mov     si, offset byte_A88
                nop
                mov     cx, 5

loc_A71:                                ; CODE XREF: COM2STAT_PARSE_PARITY+1A↓j
                cmp     al, [si]
                jz      short loc_A7F
                inc     si
                loop    loc_A71
                pop     si
COM2STAT_PARSE_PARITY endp

; START OF FUNCTION CHUNK FOR INT82H_OP_SET_PARITY
;   ADDITIONAL PARENT FUNCTION COM2STAT_PARSE_PARITY

BAD_INPUT_3:                            ; CODE XREF: COM2STAT_PARSE_PARITY+3↑j
                                        ; COM2STAT_PARSE_PARITY+5↑j ...
                xor     ah, ah
                mov     al, 3
                stc
                retn
; END OF FUNCTION CHUNK FOR INT82H_OP_SET_PARITY
; ---------------------------------------------------------------------------
; START OF FUNCTION CHUNK FOR COM2STAT_PARSE_PARITY

loc_A7F:                                ; CODE XREF: COM2STAT_PARSE_PARITY+17↑j
                pop     si
                dec     cl
                mov     al, cl
                call    INT82H_OP_SET_PARITY
                retn
; END OF FUNCTION CHUNK FOR COM2STAT_PARSE_PARITY
; ---------------------------------------------------------------------------
byte_A88        db 53h, 4Dh, 45h, 4Fh, 4Eh ; DATA XREF: COM2STAT_PARSE_PARITY+E↑o

; =============== S U B R O U T I N E =======================================


INT82H_OP_SET_PARITY proc near          ; CODE XREF: COM2STAT_PARSE_PARITY+28↑p
                                        ; INT82H_HANDLER+38↓p
                                        ; DATA XREF: ...

; FUNCTION CHUNK AT 0A79 SIZE 00000006 BYTES

                mov     bh, ds:SHADOW_MODEM_OPTIONS.BITS_AND_PARITY ; Data bits, stop bits, parity
                and     bh, 11000111b
                mov     ah, 110000b
                cmp     al, 4           ; Space
                jz      short SET_PARITY
                mov     ah, 111000b
                cmp     al, 3           ; Mark
                jz      short SET_PARITY
                mov     ah, 10000b
                cmp     al, 2           ; Even
                jz      short SET_PARITY
                mov     ah, 11000b
                cmp     al, 1           ; Odd
                jz      short SET_PARITY
                mov     ah, bh
                cmp     al, 0           ; None
                jnz     short BAD_INPUT_3

SET_PARITY:                             ; CODE XREF: INT82H_OP_SET_PARITY+B↑j
                                        ; INT82H_OP_SET_PARITY+11↑j ...
                or      ah, bh
                mov     al, 2
                call    MODEM_WRITE_REG_CRIT
                clc
                retn
INT82H_OP_SET_PARITY endp


; =============== S U B R O U T I N E =======================================


COM2STAT_BUILD_STATUS_REPLY proc near   ; CODE XREF: INT82H_OP_READ_STATUS↓p
                                        ; DATA XREF: COM2STAT_OUTPUT_IMPL+63↑o
                push    es
                push    cs
                pop     es
                assume es:seg000
                push    si
                push    cx
                mov     di, offset word_68
                mov     si, di
                mov     cx, 4
                mov     al, 0
                rep stosb
                mov     di, offset MODEM_STATE_BITS
                test    word ptr [di], MDM_ONLINE
                jz      short loc_AD9
                or      word ptr [si], 8000h

loc_AD9:                                ; CODE XREF: COM2STAT_BUILD_STATUS_REPLY+18↑j
                test    word ptr [di], MDM_CARRIER_DETECTED
                jz      short loc_AE3
                or      word ptr [si], 4000h

loc_AE3:                                ; CODE XREF: COM2STAT_BUILD_STATUS_REPLY+22↑j
                test    word ptr [di], MDM_RING_SEEN
                jz      short loc_AED
                or      word ptr [si], 2000h

loc_AED:                                ; CODE XREF: COM2STAT_BUILD_STATUS_REPLY+2C↑j
                test    word ptr [di], 800h
                jz      short loc_AFB
                or      word ptr [si], 1000h
                and     word ptr [di], 0F7FFh

loc_AFB:                                ; CODE XREF: COM2STAT_BUILD_STATUS_REPLY+36↑j
                test    word ptr [di], 200h
                jz      short loc_B09
                or      word ptr [si], 800h
                and     word ptr [di], 0FDFFh

loc_B09:                                ; CODE XREF: COM2STAT_BUILD_STATUS_REPLY+44↑j
                test    word ptr [di], 400h
                jz      short loc_B17
                or      word ptr [si], 400h
                and     word ptr [di], 0FBFFh

loc_B17:                                ; CODE XREF: COM2STAT_BUILD_STATUS_REPLY+52↑j
                mov     bx, ds:RXBUF_WRITE_CURSOR
                sub     bx, ds:RXBUF_READ_CURSOR
                jns     short loc_B25
                add     bx, ds:RXBUF_SIZE

loc_B25:                                ; CODE XREF: COM2STAT_BUILD_STATUS_REPLY+64↑j
                mov     [si+2], bx
                pop     cx
                pop     si
                pop     es
                assume es:nothing
                clc
                retn
COM2STAT_BUILD_STATUS_REPLY endp


; =============== S U B R O U T I N E =======================================


COM2STAT_PARSE_STOP_BITS proc near      ; DATA XREF: COM2STAT_OUTPUT_IMPL+5A↑o

; FUNCTION CHUNK AT 0B4B SIZE 00000006 BYTES

                call    SEARCH_EQUALS_SIGN
                jnz     short INT82H_BAD_INPUT
                jcxz    short INT82H_BAD_INPUT
                lods    byte ptr es:[si]
                dec     cx
                mov     ah, al
                mov     al, 2
                cmp     ah, '2'
                jz      short loc_B47
                dec     al
                cmp     ah, '1'
                jnz     short INT82H_BAD_INPUT

loc_B47:                                ; CODE XREF: COM2STAT_PARSE_STOP_BITS+11↑j
                call    INT82H_OP_SET_STOP_BITS
                retn
COM2STAT_PARSE_STOP_BITS endp

; ---------------------------------------------------------------------------
; START OF FUNCTION CHUNK FOR INT82H_OP_SET_STOP_BITS
;   ADDITIONAL PARENT FUNCTION COM2STAT_PARSE_STOP_BITS

INT82H_BAD_INPUT:                       ; CODE XREF: COM2STAT_PARSE_STOP_BITS+3↑j
                                        ; COM2STAT_PARSE_STOP_BITS+5↑j ...
                xor     ah, ah
                mov     al, 3
                stc
                retn
; END OF FUNCTION CHUNK FOR INT82H_OP_SET_STOP_BITS

; =============== S U B R O U T I N E =======================================


INT82H_OP_SET_STOP_BITS proc near       ; CODE XREF: COM2STAT_PARSE_STOP_BITS:loc_B47↑p
                                        ; INT82H_HANDLER+38↓p
                                        ; DATA XREF: ...

; FUNCTION CHUNK AT 0B4B SIZE 00000006 BYTES

                mov     bh, ds:SHADOW_MODEM_OPTIONS.BITS_AND_PARITY ; Data bits, stop bits, parity
                and     bh, 11111011b
                mov     ah, 100b
                cmp     al, 2           ; Two stop bits
                jz      short SET_STOP_BITS
                cmp     al, 1           ; One stop bit
                jnz     short INT82H_BAD_INPUT
                mov     ah, bh

SET_STOP_BITS:                          ; CODE XREF: INT82H_OP_SET_STOP_BITS+B↑j
                or      ah, bh
                mov     al, 2
                call    MODEM_WRITE_REG_CRIT
                clc
                retn
INT82H_OP_SET_STOP_BITS endp


; =============== S U B R O U T I N E =======================================


COM2STAT_PARSE_VOLUME proc near         ; DATA XREF: COM2STAT_OUTPUT_IMPL+AE↑o
                call    SEARCH_EQUALS_SIGN
                jnz     short ERROR
                jcxz    short ERROR
                call    PROCESS_S_REG
                mov     dx, ax
                call    INT82H_OP_SET_VOLUME
                retn
; ---------------------------------------------------------------------------

ERROR:                                  ; CODE XREF: COM2STAT_PARSE_VOLUME+3↑j
                                        ; COM2STAT_PARSE_VOLUME+5↑j
                xor     ah, ah
                mov     al, 3
                stc
                retn
COM2STAT_PARSE_VOLUME endp


; =============== S U B R O U T I N E =======================================


INT82H_OP_SET_VOLUME proc near          ; CODE XREF: COM2STAT_PARSE_VOLUME+C↑p
                                        ; INT82H_HANDLER+38↓p
                                        ; DATA XREF: ...
                mov     ax, dx
                cmp     ax, 0FFh
                jbe     short SET_VOLUME
                mov     ax, 0FFh

SET_VOLUME:                             ; CODE XREF: INT82H_OP_SET_VOLUME+5↑j
                mov     ds:VOLUME_VALUE, al
                test    ds:HAYES_STATE, 8000h
                jz      short DONE
                call    MODEM_SET_VOLUME

DONE:                                   ; CODE XREF: INT82H_OP_SET_VOLUME+13↑j
                clc
                retn
INT82H_OP_SET_VOLUME endp


; =============== S U B R O U T I N E =======================================


INT82H_HANDLER  proc far                ; DATA XREF: REG_INTERRUPT_82H+A↓o
                sti
                push    bp
                push    si
                mov     bp, ss
                mov     si, sp
                push    cs
                pop     ss
                mov     sp, 294h
                push    bp
                push    si
                pushf
                push    ax
                push    bx
                push    cx
                push    dx
                push    di
                push    ds
                push    es
                mov     bp, sp
                push    cs
                pop     ds
                assume ds:seg000
                cmp     cs:REENT_FLAG?, 0
                jnz     short loc_BDC
                cmp     ah, 15
                jbe     short loc_BC9
                mov     al, 3
                jmp     short loc_BDE
; ---------------------------------------------------------------------------
                nop

loc_BC9:                                ; CODE XREF: INT82H_HANDLER+25↑j
                mov     bl, ah
                xor     bh, bh
                shl     bx, 1
                add     bx, offset INT82H_JUMP_TABLE
                mov     bx, [bx]
                call    bx
                jb      short loc_BDE
                jmp     short loc_BEB
; ---------------------------------------------------------------------------
                align 2

loc_BDC:                                ; CODE XREF: INT82H_HANDLER+20↑j
                mov     al, 1

loc_BDE:                                ; CODE XREF: INT82H_HANDLER+29↑j
                                        ; INT82H_HANDLER+3A↑j
                or      word ptr [bp+0Eh], 1
                xor     ah, ah
                mov     [bp+0Ch], ax
                jmp     short loc_BF0
; ---------------------------------------------------------------------------
                nop

loc_BEB:                                ; CODE XREF: INT82H_HANDLER+3C↑j
                and     word ptr [bp+0Eh], 0FFFEh

loc_BF0:                                ; CODE XREF: INT82H_HANDLER+4B↑j
                pop     es
                pop     ds
                assume ds:nothing
                pop     di
                pop     dx
                pop     cx
                pop     bx
                pop     ax
                popf
                pop     si
                pop     bp
                mov     ss, bp
                mov     sp, si
                pop     si
                pop     bp
                retf    2
INT82H_HANDLER  endp


; =============== S U B R O U T I N E =======================================


REG_INTERRUPT_82H proc near             ; CODE XREF: COM2_REQ_INIT+77↓p
                push    es
                push    di
                mov     ax, 0
                mov     es, ax
                assume es:seg000
                mov     di, offset byte_208
                mov     word ptr es:[di], offset INT82H_HANDLER
                mov     word ptr es:[di+2], cs
                pop     di
                pop     es
                assume es:nothing
                retn
REG_INTERRUPT_82H endp


; =============== S U B R O U T I N E =======================================


INT82H_OP_INIT  proc near               ; CODE XREF: INT82H_HANDLER+38↑p
                                        ; DATA XREF: seg000:INT82H_JUMP_TABLE↓o
                call    RESET
                mov     word ptr [bp+0Ch], 0
                clc
                retn
INT82H_OP_INIT  endp


; =============== S U B R O U T I N E =======================================


INT82H_OP_READ  proc near               ; CODE XREF: INT82H_HANDLER+38↑p
                                        ; DATA XREF: seg000:INT82H_JUMP_TABLE↓o
                call    COM2_INPUT_IMPL
                jb      short loc_C2A
                mov     ax, cx

loc_C2A:                                ; CODE XREF: INT82H_OP_READ+3↑j
                mov     [bp+0Ch], ax
                retn
INT82H_OP_READ  endp


; =============== S U B R O U T I N E =======================================


INT82H_OP_WRITE proc near               ; CODE XREF: INT82H_HANDLER+38↑p
                                        ; DATA XREF: seg000:INT82H_JUMP_TABLE↓o
                call    COM2_OUTPUT_IMPL
                jb      short loc_C35
                mov     ax, cx

loc_C35:                                ; CODE XREF: INT82H_OP_WRITE+3↑j
                mov     [bp+0Ch], ax
                retn
INT82H_OP_WRITE endp


; =============== S U B R O U T I N E =======================================


INT82H_OP_READ_QUERY proc near          ; CODE XREF: INT82H_HANDLER+38↑p
                                        ; DATA XREF: seg000:INT82H_JUMP_TABLE↓o
                call    COM2STAT_INPUT_IMPL
                jb      short loc_C40
                mov     ax, cx

loc_C40:                                ; CODE XREF: INT82H_OP_READ_QUERY+3↑j
                mov     [bp+0Ch], ax
                retn
INT82H_OP_READ_QUERY endp


; =============== S U B R O U T I N E =======================================


INT82H_OP_WRITE_COMMAND proc near       ; CODE XREF: INT82H_HANDLER+38↑p
                                        ; DATA XREF: seg000:INT82H_JUMP_TABLE↓o
                call    COM2STAT_OUTPUT_IMPL
                jb      short loc_C4B
                mov     ax, cx

loc_C4B:                                ; CODE XREF: INT82H_OP_WRITE_COMMAND+3↑j
                mov     [bp+0Ch], ax
                retn
INT82H_OP_WRITE_COMMAND endp


; =============== S U B R O U T I N E =======================================


INT82H_OP_READ_STATUS proc near         ; CODE XREF: INT82H_HANDLER+38↑p
                                        ; DATA XREF: seg000:INT82H_JUMP_TABLE↓o
                call    COM2STAT_BUILD_STATUS_REPLY
                call    IS_RX_BUFFER_EMPTY
                mov     dx, 0
                cmc
                rcl     dx, 1
                push    dx
                call    IS_STATE_100h_FLAG
                pop     dx
                cmc
                rcl     dx, 1
                mov     cx, ds:word_6A
                mov     ax, ds:word_68
                mov     [bp+6], dx
                mov     [bp+0Ch], ax
                mov     [bp+8], cx
                clc
                retn
INT82H_OP_READ_STATUS endp


; =============== S U B R O U T I N E =======================================


INT82H_OP_RETURN_SETTINGS proc near     ; CODE XREF: INT82H_HANDLER+38↑p
                                        ; DATA XREF: seg000:INT82H_JUMP_TABLE↓o
                mov     al, ds:SHADOW_MODEM_OPTIONS.BAUD_RATE_AND_DELAY
                mov     ah, 7
                and     al, 4
                jz      short loc_C80
                mov     ah, 5

loc_C80:                                ; CODE XREF: INT82H_OP_RETURN_SETTINGS+7↑j
                mov     al, ds:SHADOW_MODEM_OPTIONS.BITS_AND_PARITY ; Data bits, stop bits, parity
                mov     ch, al
                and     ch, 3
                add     ch, 5
                mov     cl, al
                and     cl, 4
                shr     cl, 1
                shr     cl, 1
                inc     cl
                mov     dl, al
                mov     al, 0
                and     dl, 38h
                jz      short loc_CB6
                inc     al
                cmp     dl, 18h
                jz      short loc_CB6
                inc     al
                cmp     dl, 10h
                jz      short loc_CB6
                inc     al
                cmp     dl, 38h
                jz      short loc_CB6
                inc     al

loc_CB6:                                ; CODE XREF: INT82H_OP_RETURN_SETTINGS+28↑j
                                        ; INT82H_OP_RETURN_SETTINGS+2F↑j ...
                mov     [bp+8], cx
                mov     [bp+0Ch], ax
                clc
                retn
INT82H_OP_RETURN_SETTINGS endp

; ---------------------------------------------------------------------------
INT82H_JUMP_TABLE dw offset INT82H_OP_INIT ; DATA XREF: INT82H_HANDLER+32↑o
                dw offset INT82H_OP_READ ; List of methods is obtained from
                dw offset INT82H_OP_WRITE ; GRiD MS-DOS Release 2.11 Version B Update.pdf
                dw offset INT82H_OP_READ_QUERY
                dw offset INT82H_OP_WRITE_COMMAND
                dw offset INT82H_OP_READ_STATUS
                dw offset INT82H_OP_BUFFER_FLUSH
                dw offset INT82H_OP_RETURN_SETTINGS
                dw offset INT82H_OP_SET_BAUD_RATE
                dw offset INT82H_OP_SET_PARITY
                dw offset INT82H_OP_SET_DATA_BITS
                dw offset INT82H_OP_SET_STOP_BITS
                dw offset INT82H_OP_SET_RX_BUFFER
                dw offset INT82H_OP_SET_READ_CHAR_TIMEOUT
                dw offset INT82H_OP_HIGH_SPEED_DELAY
                dw offset INT82H_OP_SET_VOLUME

; =============== S U B R O U T I N E =======================================

; Attributes: bp-based frame

; int __stdcall FORMAT_BYTE_AS_3_DIGITS(char *dst, unsigned __int8 byte)
FORMAT_BYTE_AS_3_DIGITS proc near       ; CODE XREF: seg000:11A8↓p

dst             = word ptr  4
byte            = byte ptr  6

                push    bp
                mov     bp, sp
                push    bx
                push    cx
                push    dx
                push    di
                push    es
                push    ds
                pop     es
                mov     di, [bp+dst]
                mov     al, '0'
                mov     cx, 3
                rep stosb
                mov     dx, word ptr [bp+byte]
                mov     di, [bp+dst]
                cmp     dl, 0
                jz      short EPILOGUE
                mov     ax, 100

LOOP_START:                             ; CODE XREF: FORMAT_BYTE_AS_3_DIGITS+43↓j
                xor     cx, cx

loc_D02:                                ; CODE XREF: FORMAT_BYTE_AS_3_DIGITS+2B↓j
                sub     dl, al
                jns     short loc_D08
                jmp     short loc_D0B
; ---------------------------------------------------------------------------

loc_D08:                                ; CODE XREF: FORMAT_BYTE_AS_3_DIGITS+26↑j
                inc     cx
                jmp     short loc_D02
; ---------------------------------------------------------------------------

loc_D0B:                                ; CODE XREF: FORMAT_BYTE_AS_3_DIGITS+28↑j
                jz      short loc_D11
                add     dl, al
                jmp     short loc_D12
; ---------------------------------------------------------------------------

loc_D11:                                ; CODE XREF: FORMAT_BYTE_AS_3_DIGITS:loc_D0B↑j
                inc     cx

loc_D12:                                ; CODE XREF: FORMAT_BYTE_AS_3_DIGITS+31↑j
                or      cl, 110000b
                mov     [di], cl
                inc     di
                cmp     dl, 0
                jz      short EPILOGUE
                mov     bl, 10
                div     bl
                jmp     short LOOP_START
; ---------------------------------------------------------------------------

EPILOGUE:                               ; CODE XREF: FORMAT_BYTE_AS_3_DIGITS+1D↑j
                                        ; FORMAT_BYTE_AS_3_DIGITS+3D↑j
                pop     es
                pop     di
                pop     dx
                pop     cx
                pop     bx
                pop     bp
                retn    4
FORMAT_BYTE_AS_3_DIGITS endp


; =============== S U B R O U T I N E =======================================


PROCESS_S_REG   proc near               ; CODE XREF: COM2STAT_PARSE_READ_TIMEOUT+3↑p
                                        ; COM2STAT_PARSE_VOLUME+7↑p ...
                push    di
                push    bx
                push    cx
                push    ds
                mov     di, si
                cmp     cx, 6
                jb      short loc_D3A
                mov     cx, 6

loc_D3A:                                ; CODE XREF: PROCESS_S_REG+9↑j
                mov     bx, 0

loc_D3D:                                ; CODE XREF: PROCESS_S_REG+21↓j
                mov     al, es:[bx+di]
                cmp     al, '0'
                jb      short loc_D4F
                cmp     al, '9'
                ja      short loc_D4F
                inc     bx
                cmp     bx, cx
                jnb     short loc_D4F
                jmp     short loc_D3D
; ---------------------------------------------------------------------------

loc_D4F:                                ; CODE XREF: PROCESS_S_REG+16↑j
                                        ; PROCESS_S_REG+1A↑j ...
                cmp     bx, 0
                jz      short loc_D87
                mov     cx, bx
                dec     bx
                add     di, bx
                xchg    si, di
                mov     ax, cs
                mov     ds, ax
                assume ds:seg000
                mov     bx, 1
                xor     dx, dx
                push    dx
                std

loc_D66:                                ; CODE XREF: PROCESS_S_REG+54↓j
                xor     ax, ax
                lodsb
                cmp     al, '0'
                jz      short loc_D79
                and     al, 0Fh
                mul     bx
                jb      short loc_D86
                pop     dx
                add     dx, ax
                jb      short loc_D87
                push    dx

loc_D79:                                ; CODE XREF: PROCESS_S_REG+3F↑j
                mov     ax, 10
                mul     bx
                mov     bx, ax
                loop    loc_D66
                pop     ax
                clc
                jmp     short loc_D88
; ---------------------------------------------------------------------------

loc_D86:                                ; CODE XREF: PROCESS_S_REG+45↑j
                pop     dx

loc_D87:                                ; CODE XREF: PROCESS_S_REG+26↑j
                                        ; PROCESS_S_REG+4A↑j
                stc

loc_D88:                                ; CODE XREF: PROCESS_S_REG+58↑j
                cld
                xchg    si, di
                pop     ds
                assume ds:nothing
                pop     cx
                pop     bx
                pop     di
                retn
PROCESS_S_REG   endp


; =============== S U B R O U T I N E =======================================


CONVERT_CHAR_TO_UPPERCASE proc near     ; CODE XREF: COM2STAT_OUTPUT_IMPL+41↑p
                                        ; COM2STAT_OUTPUT_IMPL+7A↑p ...
                cmp     al, 'a'
                jb      short NOT_ALPHA
                cmp     al, 'z'
                ja      short NOT_ALPHA
                and     al, 11011111b

NOT_ALPHA:                              ; CODE XREF: CONVERT_CHAR_TO_UPPERCASE+2↑j
                                        ; CONVERT_CHAR_TO_UPPERCASE+6↑j
                retn
CONVERT_CHAR_TO_UPPERCASE endp


; =============== S U B R O U T I N E =======================================


SEARCH_EQUALS_SIGN proc near            ; CODE XREF: COM2STAT_PARSE_BAUD_RATE↑p
                                        ; COM2STAT_PARSE_READ_TIMEOUT↑p ...
                push    ax
                mov     al, '='
                mov     di, si
                repne scasb
                mov     si, di
                pop     ax
                retn
SEARCH_EQUALS_SIGN endp


; =============== S U B R O U T I N E =======================================


MODEM_READ_STATUS proc near             ; CODE XREF: MODEM_INT_HANDLER+12↑p
                                        ; MODEM_INT_HANDLER+26↑p ...
                push    es
                push    bx
                mov     bx, 0DFE0h
                mov     es, bx
                assume es:nothing

WAIT_READY:                             ; CODE XREF: MODEM_READ_STATUS+D↓j
                test    es:MODEM_STATUS_C4, 80h
                jz      short WAIT_READY
                and     al, 11b
                add     al, 11000000b
                mov     ah, 5
                mov     es:MODEM_COMMAND_C6, ah
                mov     es:MODEM_DATA_C0, al

WAIT_ACCEPT:                            ; CODE XREF: MODEM_READ_STATUS+24↓j
                test    es:MODEM_STATUS_C4, 80h
                jz      short WAIT_ACCEPT
                mov     ah, 4
                mov     es:MODEM_COMMAND_C6, ah

WAIT_DATA:                              ; CODE XREF: MODEM_READ_STATUS+33↓j
                test    es:MODEM_STATUS_C4, 20h
                jz      short WAIT_DATA
                mov     ah, es:MODEM_DATA_C0
                pop     bx
                pop     es
                assume es:nothing
                retn
MODEM_READ_STATUS endp


; =============== S U B R O U T I N E =======================================


MODEM_WRITE_REG proc near               ; CODE XREF: MODEM_INT_HANDLER:WRITE_OPTION_TO_MODEM↑p
                                        ; MODEM_WRITE_REG_CRIT+1↓p
                push    es
                push    bx
                push    si
                push    ax
                mov     bx, 0DFE0h
                mov     es, bx
                assume es:nothing

loc_DEC:                                ; CODE XREF: MODEM_WRITE_REG+F↓j
                test    es:MODEM_STATUS_C4, 80h
                jz      short loc_DEC
                xor     bh, bh
                mov     si, 68
                mov     bl, al
                mov     [bx+si], ah
                and     al, 11111b
                add     al, 80h
                mov     bl, 5
                mov     es:MODEM_COMMAND_C6, bl
                mov     es:MODEM_DATA_C0, al

loc_E0C:                                ; CODE XREF: MODEM_WRITE_REG+2F↓j
                test    es:MODEM_STATUS_C4, 80h
                jz      short loc_E0C
                mov     bl, 4
                mov     es:MODEM_COMMAND_C6, bl
                mov     es:MODEM_DATA_C0, ah

loc_E20:                                ; CODE XREF: MODEM_WRITE_REG+43↓j
                test    es:MODEM_STATUS_C4, 80h
                jz      short loc_E20
                pop     ax
                pop     si
                pop     bx
                pop     es
                assume es:nothing
                retn
MODEM_WRITE_REG endp


; =============== S U B R O U T I N E =======================================


MODEM_EXEC_COMMAND proc near            ; CODE XREF: MODEM_INT_HANDLER+10F↑p
                                        ; MODEM_EXEC_COMMAND_CRIT+1↓p
                push    es
                push    bx
                mov     bx, 0DFE0h
                mov     es, bx
                assume es:nothing

WAIT_READY_FOR_COMMAND:                 ; CODE XREF: MODEM_EXEC_COMMAND+D↓j
                test    es:MODEM_STATUS_C4, 80h
                jz      short WAIT_READY_FOR_COMMAND
                mov     ah, 5
                mov     es:MODEM_COMMAND_C6, ah
                mov     es:MODEM_DATA_C0, al

WAIT_ACCEPT:                            ; CODE XREF: MODEM_EXEC_COMMAND+20↓j
                test    es:MODEM_STATUS_C4, 80h
                jz      short WAIT_ACCEPT
                mov     ah, 4
                mov     es:MODEM_COMMAND_C6, ah
                pop     bx
                pop     es
                assume es:nothing
                retn
MODEM_EXEC_COMMAND endp


; =============== S U B R O U T I N E =======================================


MODEM_READ_STATUS_CRIT proc near        ; CODE XREF: COM2_OUTPUT_IMPL+4D↑p
                                        ; GO_OFF_HOOK:WAIT_MODEM↓p ...
                cli
                call    MODEM_READ_STATUS
                sti
                retn
MODEM_READ_STATUS_CRIT endp


; =============== S U B R O U T I N E =======================================


MODEM_WRITE_REG_CRIT proc near          ; CODE XREF: COM2_OUTPUT_IMPL+7A↑p
                                        ; INT82H_OP_SET_BAUD_RATE+16↑p ...
                cli
                call    MODEM_WRITE_REG
                sti
                retn
MODEM_WRITE_REG_CRIT endp


; =============== S U B R O U T I N E =======================================


MODEM_EXEC_COMMAND_CRIT proc near       ; CODE XREF: MODEM_DISABLE_CARRIER_UNCOND+F↓p
                                        ; START_HANDSHAKE_AND_WAIT_CONNECT+34↓p ...
                cli
                call    MODEM_EXEC_COMMAND
                sti
                retn
MODEM_EXEC_COMMAND_CRIT endp


; =============== S U B R O U T I N E =======================================


PROCESS_INPUT_A proc near               ; CODE XREF: PROCESS_INPUT+48↓p
                and     ds:MODEM_STATE_BITS, 0DFFFh
                mov     ax, 0
                mov     ds:REG_S1, ax
                mov     ax, 4
                push    ax
                push    ax
                push    ax
                call    DISPATCH_MODEM_ACTION
                jb      short locret_E93
                test    ds:MODEM_STATE_BITS, 8
                jnz     short locret_E93
                mov     ax, 3
                push    ax
                push    ax
                push    ax
                call    DISPATCH_MODEM_ACTION

locret_E93:                             ; CODE XREF: PROCESS_INPUT_A+15↑j
                                        ; PROCESS_INPUT_A+1D↑j
                retn
PROCESS_INPUT_A endp


; =============== S U B R O U T I N E =======================================


PROCESS_INPUT_C proc near               ; CODE XREF: PROCESS_INPUT+58↓p
                mov     al, [si]
                cmp     al, '0'
                jnz     short CHECK_1
                inc     si              ; Next char
                dec     cx
                test    ds:MODEM_STATE_BITS, 80h
                jz      short DISABLE_TRANSMITTER ; Disable carrier
                mov     ax, 2           ; Disable carrier unconditionally
                push    ax
                push    ax
                push    ax
                call    DISPATCH_MODEM_ACTION

DISABLE_TRANSMITTER:                    ; CODE XREF: PROCESS_INPUT_C+E↑j
                mov     ax, 8           ; Disable carrier
                push    ax
                push    ax
                push    ax
                call    DISPATCH_MODEM_ACTION
                jmp     short DONE
; ---------------------------------------------------------------------------

CHECK_1:                                ; CODE XREF: PROCESS_INPUT_C+4↑j
                cmp     al, '1'
                jnz     short ENABLE_TRANSMITTER ; Enable carrier
                inc     si              ; Next char
                dec     cx

ENABLE_TRANSMITTER:                     ; CODE XREF: PROCESS_INPUT_C+26↑j
                mov     ax, 9           ; Enable carrier
                push    ax
                push    ax
                push    ax
                call    DISPATCH_MODEM_ACTION
                mov     ax, 3           ; Start handshake and wait connect
                push    ax
                push    ax
                push    ax
                call    DISPATCH_MODEM_ACTION

DONE:                                   ; CODE XREF: PROCESS_INPUT_C+22↑j
                retn
PROCESS_INPUT_C endp


; =============== S U B R O U T I N E =======================================


PROCESS_INPUT_D proc near               ; CODE XREF: PROCESS_INPUT+68↓p
                push    di
                push    es
                push    bx
                push    cx
                xor     bx, bx
                mov     ax, ds
                mov     es, ax

LOOP_START:                             ; CODE XREF: PROCESS_INPUT_D+21↓j
                mov     al, [bx+si]
                cmp     al, ','
                jnz     short loc_EE7
                mov     al, '.'
                mov     [bx+si], al
                jmp     short loc_EF1
; ---------------------------------------------------------------------------

loc_EE7:                                ; CODE XREF: PROCESS_INPUT_D+E↑j
                mov     cx, 14h
                mov     di, offset a0123456789Pt ; "0123456789*#.()-^ PT"
                repne scasb
                jnz     short CHECK_R

loc_EF1:                                ; CODE XREF: PROCESS_INPUT_D+14↑j
                inc     bx
                jmp     short LOOP_START
; ---------------------------------------------------------------------------

CHECK_R:                                ; CODE XREF: PROCESS_INPUT_D+1E↑j
                cmp     al, 'R'
                jnz     short CHECK_SEMICOLON
                mov     al, 2
                mov     ds:byte_6E, al
                jmp     short loc_F03
; ---------------------------------------------------------------------------

CHECK_SEMICOLON:                        ; CODE XREF: PROCESS_INPUT_D+25↑j
                cmp     al, ';'
                jz      short SEMICOLON_FOUND

loc_F03:                                ; CODE XREF: PROCESS_INPUT_D+2C↑j
                and     ds:STATE_BITS, 7FFFh

SEMICOLON_FOUND:                        ; CODE XREF: PROCESS_INPUT_D+30↑j
                dec     si
                mov     [si], bl
                test    ds:HAYES_STATE, 0C000h
                jnz     short DIAL_NUMBER ; Dial number
                mov     al, ds:VOLUME_VALUE
                call    MODEM_SET_VOLUME

DIAL_NUMBER:                            ; CODE XREF: PROCESS_INPUT_D+41↑j
                mov     ax, 7           ; Dial number
                push    ax
                push    ds
                push    si
                call    DISPATCH_MODEM_ACTION
                test    ds:STATE_BITS, 100h
                jnz     short GO_ON_HOOK ; Go on hook
                test    ds:STATE_BITS, 8000h
                jnz     short loc_F5D
                test    ds:MODEM_STATE_BITS, MDM_CARRIER_DISABLED
                jnz     short loc_F5D
                mov     ax, 3           ; Start handshake and wait connect
                push    ax
                push    ax
                push    ax
                call    DISPATCH_MODEM_ACTION
                jb      short GO_ON_HOOK ; Go on hook
                test    ds:STATE_BITS, 100h
                jz      short loc_F5D

GO_ON_HOOK:                             ; CODE XREF: PROCESS_INPUT_D+58↑j
                                        ; PROCESS_INPUT_D+73↑j
                mov     ax, 5           ; Go on hook
                push    ax
                push    ax
                push    ax
                call    DISPATCH_MODEM_ACTION
                or      ds:STATE_BITS, 8000h

loc_F5D:                                ; CODE XREF: PROCESS_INPUT_D+60↑j
                                        ; PROCESS_INPUT_D+68↑j ...
                mov     al, 'D'
                mov     [si], al
                inc     si
                inc     bx
                add     si, bx
                test    ds:HAYES_STATE, 0C000h
                jnz     short DONE
                mov     al, 0
                call    MODEM_SET_VOLUME

DONE:                                   ; CODE XREF: PROCESS_INPUT_D+9A↑j
                pop     cx
                sub     cx, bx
                pop     bx
                pop     es
                pop     di
                retn
PROCESS_INPUT_D endp


; =============== S U B R O U T I N E =======================================


PROCESS_INPUT_F proc near               ; CODE XREF: PROCESS_INPUT+82↓p
                mov     al, [si]
                cmp     al, '0'
                jnz     short CHECK_1
                inc     si
                dec     cx
                or      ds:HAYES_STATE, 10000000b
                jmp     short DONE
; ---------------------------------------------------------------------------

CHECK_1:                                ; CODE XREF: PROCESS_INPUT_F+4↑j
                cmp     al, '1'
                jnz     short BAD_INPUT
                inc     si
                dec     cx

BAD_INPUT:                              ; CODE XREF: PROCESS_INPUT_F+12↑j
                and     ds:HAYES_STATE, 1111111101111111b

DONE:                                   ; CODE XREF: PROCESS_INPUT_F+E↑j
                retn
PROCESS_INPUT_F endp


; =============== S U B R O U T I N E =======================================


PROCESS_INPUT_E proc near               ; CODE XREF: PROCESS_INPUT+79↓p
                mov     al, [si]
                cmp     al, '1'
                jnz     short loc_FA6
                inc     si
                dec     cx
                or      ds:HAYES_STATE, 100000000b
                jmp     short locret_FB2
; ---------------------------------------------------------------------------

loc_FA6:                                ; CODE XREF: PROCESS_INPUT_E+4↑j
                cmp     al, '0'
                jnz     short loc_FAC
                inc     si
                dec     cx

loc_FAC:                                ; CODE XREF: PROCESS_INPUT_E+12↑j
                and     ds:HAYES_STATE, 1111111011111111b

locret_FB2:                             ; CODE XREF: PROCESS_INPUT_E+E↑j
                retn
PROCESS_INPUT_E endp


; =============== S U B R O U T I N E =======================================


PROCESS_INPUT_H proc near               ; CODE XREF: PROCESS_INPUT+8B↓p
                mov     al, [si]
                cmp     al, '1'
                jnz     short CHECK_0
                inc     si              ; Next char
                dec     cx
                mov     ax, 4           ; Off-hook
                jmp     short DO
; ---------------------------------------------------------------------------

CHECK_0:                                ; CODE XREF: PROCESS_INPUT_H+4↑j
                cmp     al, '0'
                jnz     short ON_HOOK
                inc     si              ; Next char
                dec     cx

ON_HOOK:                                ; CODE XREF: PROCESS_INPUT_H+F↑j
                mov     ax, 5

DO:                                     ; CODE XREF: PROCESS_INPUT_H+B↑j
                push    ax
                push    ax
                push    ax
                call    DISPATCH_MODEM_ACTION
                retn
PROCESS_INPUT_H endp


; =============== S U B R O U T I N E =======================================


PROCESS_INPUT_O proc near               ; CODE XREF: PROCESS_INPUT+9D↓p
                and     ds:STATE_BITS, 7DFFh
                retn
PROCESS_INPUT_O endp


; =============== S U B R O U T I N E =======================================


PROCESS_INPUT_P proc near               ; CODE XREF: PROCESS_INPUT+A7↓p
                and     ds:STATE_BITS, 0EFFFh
                retn
PROCESS_INPUT_P endp


; =============== S U B R O U T I N E =======================================


PROCESS_INPUT_Q proc near               ; CODE XREF: PROCESS_INPUT+B0↓p
                mov     al, [si]
                cmp     al, '0'
                jnz     short loc_FEE
                inc     si
                dec     cx
                and     ds:HAYES_STATE, 1110111111111111b
                jmp     short locret_FFA
; ---------------------------------------------------------------------------

loc_FEE:                                ; CODE XREF: PROCESS_INPUT_Q+4↑j
                cmp     al, '1'
                jnz     short loc_FF4
                inc     si
                dec     cx

loc_FF4:                                ; CODE XREF: PROCESS_INPUT_Q+12↑j
                or      ds:HAYES_STATE, 1000000000000b

locret_FFA:                             ; CODE XREF: PROCESS_INPUT_Q+E↑j
                retn
PROCESS_INPUT_Q endp


; =============== S U B R O U T I N E =======================================


PROCESS_INPUT_X proc near               ; CODE XREF: PROCESS_INPUT+DC↓p
                mov     al, [si]
                cmp     al, '1'
                jnz     short loc_100B
                inc     si
                dec     cx
                or      ds:HAYES_STATE, 400h
                jmp     short locret_1017
; ---------------------------------------------------------------------------

loc_100B:                               ; CODE XREF: PROCESS_INPUT_X+4↑j
                cmp     al, '0'
                jnz     short loc_1011
                inc     si
                dec     cx

loc_1011:                               ; CODE XREF: PROCESS_INPUT_X+12↑j
                and     ds:HAYES_STATE, 0FBFFh

locret_1017:                            ; CODE XREF: PROCESS_INPUT_X+E↑j
                retn
PROCESS_INPUT_X endp


; =============== S U B R O U T I N E =======================================


PROCESS_INPUT_S proc near               ; CODE XREF: PROCESS_INPUT:HANDLE_S↓p
                push    dx
                push    bx
                push    di
                push    es
                cmp     al, 'S'
                jnz     short loc_1042
                mov     ax, ds
                mov     es, ax
                call    PROCESS_S_REG
                cmp     al, 0
                jz      short loc_107C
                cmp     al, 10h
                jz      short loc_107C
                ja      short loc_107B
                mov     ds:S_REG_NUMBER, al

loc_1034:                               ; CODE XREF: PROCESS_INPUT_S+28↓j
                jcxz    short loc_107C
                lodsb
                dec     cx
                cmp     al, '0'
                jb      short loc_1042
                cmp     al, '9'
                ja      short loc_1042
                jmp     short loc_1034
; ---------------------------------------------------------------------------

loc_1042:                               ; CODE XREF: PROCESS_INPUT_S+6↑j
                                        ; PROCESS_INPUT_S+22↑j ...
                cmp     al, '='
                jnz     short loc_1055
                cmp     ds:S_REG_NUMBER, 12
                ja      short loc_107B
                call    PROCESS_S_REG
                mov     di, offset S_REG_WRITERS_TABLE
                jmp     short loc_105C
; ---------------------------------------------------------------------------

loc_1055:                               ; CODE XREF: PROCESS_INPUT_S+2C↑j
                cmp     al, '?'
                jnz     short loc_1076
                mov     di, offset S_REG_READERS_TABLE

loc_105C:                               ; CODE XREF: PROCESS_INPUT_S+3B↑j
                xor     bh, bh
                mov     bl, ds:S_REG_NUMBER
                shl     bx, 1
                add     di, bx
                call    word ptr [di]

loc_1068:                               ; CODE XREF: PROCESS_INPUT_S+5C↓j
                jcxz    short loc_107C
                lodsb
                dec     cx
                cmp     al, '0'
                jb      short loc_1076
                cmp     al, '9'
                ja      short loc_1076
                jmp     short loc_1068
; ---------------------------------------------------------------------------

loc_1076:                               ; CODE XREF: PROCESS_INPUT_S+3F↑j
                                        ; PROCESS_INPUT_S+56↑j ...
                inc     cx
                dec     si
                clc
                jmp     short loc_107C
; ---------------------------------------------------------------------------

loc_107B:                               ; CODE XREF: PROCESS_INPUT_S+17↑j
                                        ; PROCESS_INPUT_S+33↑j
                stc

loc_107C:                               ; CODE XREF: PROCESS_INPUT_S+11↑j
                                        ; PROCESS_INPUT_S+15↑j ...
                pop     es
                pop     di
                pop     bx
                pop     dx
                retn
PROCESS_INPUT_S endp

; ---------------------------------------------------------------------------

S1_READ:                                ; DATA XREF: seg000:0406↑o
                mov     ax, ds:REG_S1
                jmp     loc_11A1
; ---------------------------------------------------------------------------

S2_READ:                                ; DATA XREF: seg000:0408↑o
                mov     al, ds:REG_S2
                jmp     loc_11A1
; ---------------------------------------------------------------------------

S3_READ:                                ; DATA XREF: seg000:040A↑o
                mov     al, ds:REG_S3   ; Value of the carriage return.
                jmp     loc_11A1
; ---------------------------------------------------------------------------

S4_READ:                                ; DATA XREF: seg000:040C↑o
                mov     al, ds:REG_S4
                jmp     loc_11A1
; ---------------------------------------------------------------------------

S5_READ:                                ; DATA XREF: seg000:040E↑o
                mov     al, ds:REG_S5
                jmp     loc_11A1
; ---------------------------------------------------------------------------

S6_READ:                                ; DATA XREF: seg000:0410↑o
                xor     dx, dx
                mov     ax, ds:REG_S6
                mov     bx, 1000
                div     bx
                jmp     loc_11A1
; ---------------------------------------------------------------------------

S7_READ:                                ; DATA XREF: seg000:0412↑o
                mov     ax, ds:REG_S7   ; Seconds
                jmp     loc_11A1
; ---------------------------------------------------------------------------

S8_READ:                                ; DATA XREF: seg000:0414↑o
                xor     dx, dx
                mov     ax, ds:REG_S8   ; Milliseconds
                mov     bx, offset MODEM_SET_VOLUME_FROM_MEM_INDIR
                div     bx
                jmp     loc_11A1
; ---------------------------------------------------------------------------

S9_READ:                                ; DATA XREF: seg000:0416↑o
                mov     ax, ds:REG_S9   ; Milliseconds
                mov     bl, 100
                div     bl
                jmp     loc_11A1
; ---------------------------------------------------------------------------

S10_READ:                               ; DATA XREF: seg000:0418↑o
                mov     ax, ds:REG_S10  ; Time in milliseconds to wait before disconnecting after a remote hangs up.
                mov     bl, 100
                div     bl
                jmp     loc_11A1
; ---------------------------------------------------------------------------

S11_READ:                               ; DATA XREF: seg000:041A↑o
                mov     ax, ds:REG_S11  ; Speed of the touch-tone dialer. In milliseconds.
                jmp     loc_11A1
; ---------------------------------------------------------------------------

S12_READ:                               ; DATA XREF: seg000:041C↑o
                mov     ax, ds:REG_S12  ; Milliseconds
                mov     dx, ds:word_96
                mov     bx, 20
                div     bx
                jmp     loc_11A1
; ---------------------------------------------------------------------------

loc_10E8:                               ; DATA XREF: seg000:041E↑o
                xor     ax, ax
                test    ds:HAYES_STATE, 10000000000b
                jz      short loc_10F4
                or      al, 2

loc_10F4:                               ; CODE XREF: seg000:10F0↑j
                mov     dl, ds:SHADOW_MODEM_OPTIONS.BITS_AND_PARITY ; Data bits, stop bits, parity
                test    dl, 10000b      ; "even" parity check?
                jz      short loc_1114
                or      al, 4
                test    dl, 20h
                jnz     short loc_110D
                test    dl, 8
                jnz     short loc_1114
                or      al, 8
                jmp     short loc_1114
; ---------------------------------------------------------------------------

loc_110D:                               ; CODE XREF: seg000:1102↑j
                test    dl, 8
                jz      short loc_1114
                or      al, 10000000b

loc_1114:                               ; CODE XREF: seg000:10FB↑j seg000:1107↑j ...
                test    dl, 1
                jz      short loc_111B
                or      al, 10h

loc_111B:                               ; CODE XREF: seg000:1117↑j
                test    ds:MODEM_STATE_BITS, 200h
                jz      short loc_1125
                or      al, 40h

loc_1125:                               ; CODE XREF: seg000:1121↑j
                jmp     short loc_11A1
; ---------------------------------------------------------------------------
                align 2

loc_1128:                               ; DATA XREF: seg000:0420↑o
                xor     ax, ax
                mov     dx, ds:HAYES_STATE
                test    dx, 100h
                jz      short loc_1136
                or      al, 2

loc_1136:                               ; CODE XREF: seg000:1132↑j
                test    dx, 1000h
                jz      short loc_113E
                or      al, 4

loc_113E:                               ; CODE XREF: seg000:113A↑j
                test    dx, 800h
                jz      short loc_1146
                or      al, 8

loc_1146:                               ; CODE XREF: seg000:1142↑j
                test    dx, 4000h
                jnz     short loc_1156
                or      al, 40h
                test    dx, 8000h
                jz      short loc_1156
                or      al, 80h

loc_1156:                               ; CODE XREF: seg000:114A↑j seg000:1152↑j
                test    ds:STATE_BITS, 1000h
                jnz     short loc_1160
                or      al, 100000b

loc_1160:                               ; CODE XREF: seg000:115C↑j
                jmp     short loc_11A1
; ---------------------------------------------------------------------------
                nop

loc_1163:                               ; DATA XREF: seg000:0422↑o
                mov     al, '"'
                test    ds:HAYES_STATE, 80h
                jnz     short loc_116F
                or      al, 8

loc_116F:                               ; CODE XREF: seg000:116B↑j
                mov     dx, ds:8Ah
                test    dx, 80h
                jz      short loc_1183
                test    dx, 40h
                jz      short loc_118E
                or      al, 4
                jmp     short loc_118E
; ---------------------------------------------------------------------------

loc_1183:                               ; CODE XREF: seg000:1177↑j
                mov     dl, ds:byte_6E
                cmp     dl, 1
                jnz     short loc_118E
                or      al, 4

loc_118E:                               ; CODE XREF: seg000:117D↑j seg000:1181↑j ...
                test    ds:SHADOW_MODEM_OPTIONS.BAUD_RATE_AND_DELAY, 4
                jnz     short loc_1197
                or      al, 11h

loc_1197:                               ; CODE XREF: seg000:1193↑j
                test    ds:MODEM_STATE_BITS, 8000h
                jz      short loc_11A1
                or      al, 40h

loc_11A1:                               ; CODE XREF: seg000:1084↑j seg000:108A↑j ...
                xor     ah, ah
                push    ax
                mov     ax, (offset a000+3)
                push    ax
                call    FORMAT_BYTE_AS_3_DIGITS
                push    cx
                push    si
                mov     si, offset a000 ; "\r\n000\r\n"
                mov     cl, [si]
                xor     ch, ch
                inc     si
                call    RX_BUFFER_WRITE_BYTES
                pop     si
                pop     cx
                retn

; =============== S U B R O U T I N E =======================================


PROCESS_INPUT_Z proc near               ; CODE XREF: PROCESS_INPUT+E5↓p
                call    RESET
                retn
PROCESS_INPUT_Z endp

; ---------------------------------------------------------------------------

S1_WRITE:                               ; DATA XREF: seg000:03EC↑o
                xor     ah, ah
                mov     ds:REG_S1, ax
                jmp     DONE
; ---------------------------------------------------------------------------

S2_WRITE:                               ; DATA XREF: seg000:03EE↑o
                mov     ds:REG_S2, al
                jmp     DONE
; ---------------------------------------------------------------------------

S3_WRITE:                               ; DATA XREF: seg000:03F0↑o
                mov     ds:REG_S3, al   ; Value of the carriage return.
                jmp     DONE
; ---------------------------------------------------------------------------

S4_WRITE:                               ; DATA XREF: seg000:03F2↑o
                mov     ds:REG_S4, al
                jmp     DONE
; ---------------------------------------------------------------------------

S5_WRITE:                               ; DATA XREF: seg000:03F4↑o
                mov     ds:REG_S5, al
                jmp     DONE
; ---------------------------------------------------------------------------

S6_WRITE:                               ; DATA XREF: seg000:03F6↑o
                cmp     al, 2
                jnb     short loc_11E7
                mov     al, 2
                jmp     short loc_11ED
; ---------------------------------------------------------------------------

loc_11E7:                               ; CODE XREF: seg000:11E1↑j
                cmp     al, 'A'
                jbe     short loc_11ED
                mov     al, 'A'

loc_11ED:                               ; CODE XREF: seg000:11E5↑j seg000:11E9↑j
                mov     bx, offset MODEM_SET_VOLUME_FROM_MEM_INDIR
                mul     bx
                mov     ds:REG_S6, ax
                jmp     DONE
; ---------------------------------------------------------------------------

S7_WRITE:                               ; DATA XREF: seg000:03F8↑o
                xor     ah, ah
                mov     ds:REG_S7, ax   ; Seconds
                jmp     short DONE
; ---------------------------------------------------------------------------
                align 2

S8_WRITE:                               ; DATA XREF: seg000:03FA↑o
                cmp     al, 65
                jbe     short loc_1206
                mov     al, 65

loc_1206:                               ; CODE XREF: seg000:1202↑j
                mov     bx, 3E8h
                mul     bx
                mov     ds:REG_S8, ax   ; Milliseconds
                jmp     short DONE
; ---------------------------------------------------------------------------
                nop

S9_WRITE:                               ; DATA XREF: seg000:03FC↑o
                xor     ah, ah
                mov     bx, 100
                mul     bx
                mov     ds:REG_S9, ax   ; Milliseconds
                jmp     short DONE
; ---------------------------------------------------------------------------
                align 2

S10_WRITE:                              ; DATA XREF: seg000:03FE↑o
                xor     ah, ah
                cmp     al, 0
                jnz     short loc_1226
                mov     al, 7

loc_1226:                               ; CODE XREF: seg000:1222↑j
                cmp     al, 0FFh
                jnb     short loc_123D
                test    ds:MODEM_STATE_BITS, 80h
                jz      short loc_1251
                push    ax
                mov     ah, 80h
                mov     al, 6
                call    MODEM_WRITE_REG_CRIT
                pop     ax
                jmp     short loc_1251
; ---------------------------------------------------------------------------

loc_123D:                               ; CODE XREF: seg000:1228↑j
                mov     ah, ds:SHADOW_MODEM_OPTIONS.UNK_6
                and     ah, 7Fh
                mov     al, 6
                call    MODEM_WRITE_REG_CRIT
                mov     al, 0FFh
                or      ds:MODEM_STATE_BITS, 8000h

loc_1251:                               ; CODE XREF: seg000:1230↑j seg000:123B↑j
                mov     ah, al
                mov     al, 8
                call    MODEM_WRITE_REG_CRIT
                jmp     short DONE
; ---------------------------------------------------------------------------
                nop

S11_WRITE:                              ; DATA XREF: seg000:0400↑o
                cmp     ax, 50
                jnb     short loc_1263
                mov     ax, 50

loc_1263:                               ; CODE XREF: seg000:125E↑j
                mov     ds:REG_S11, ax  ; Speed of the touch-tone dialer. In milliseconds.
                jmp     short DONE
; ---------------------------------------------------------------------------
                nop

S12_WRITE:                              ; DATA XREF: seg000:0402↑o
                xor     ah, ah
                mov     bx, 14h
                mul     bx
                mov     ds:word_96, dx
                mov     ds:REG_S12, ax  ; Milliseconds

DONE:                                   ; CODE XREF: seg000:11C4↑j seg000:11CA↑j ...
                retn

; =============== S U B R O U T I N E =======================================


PROCESS_INPUT_M proc near               ; CODE XREF: PROCESS_INPUT+94↓p
                mov     al, [si]
                cmp     al, '0'
                jnz     short loc_1290
                inc     si
                dec     cx
                and     ds:HAYES_STATE, 7FFFh
                or      ds:HAYES_STATE, 4000h
                mov     al, 0
                jmp     short loc_12B5
; ---------------------------------------------------------------------------

loc_1290:                               ; CODE XREF: PROCESS_INPUT_M+4↑j
                cmp     al, '2'
                jnz     short loc_12A7
                inc     si
                dec     cx
                and     ds:HAYES_STATE, 0BFFFh
                or      ds:HAYES_STATE, 8000h
                mov     al, ds:VOLUME_VALUE
                jmp     short loc_12B5
; ---------------------------------------------------------------------------

loc_12A7:                               ; CODE XREF: PROCESS_INPUT_M+1A↑j
                cmp     al, '1'
                jnz     short loc_12AD
                inc     si
                dec     cx

loc_12AD:                               ; CODE XREF: PROCESS_INPUT_M+31↑j
                and     ds:HAYES_STATE, 3FFFh
                mov     al, 0

loc_12B5:                               ; CODE XREF: PROCESS_INPUT_M+16↑j
                                        ; PROCESS_INPUT_M+2D↑j
                call    MODEM_SET_VOLUME
                retn
PROCESS_INPUT_M endp


; =============== S U B R O U T I N E =======================================


PROCESS_INPUT_T proc near               ; CODE XREF: PROCESS_INPUT+CA↓p
                or      ds:STATE_BITS, 1000h
                retn
PROCESS_INPUT_T endp


; =============== S U B R O U T I N E =======================================


PROCESS_INPUT_V proc near               ; CODE XREF: PROCESS_INPUT+D3↓p
                mov     al, [si]
                cmp     al, '1'
                jnz     short loc_12D0
                inc     si
                dec     cx
                or      ds:HAYES_STATE, 800h
                jmp     short locret_12DC
; ---------------------------------------------------------------------------

loc_12D0:                               ; CODE XREF: PROCESS_INPUT_V+4↑j
                cmp     al, '0'
                jnz     short loc_12D6
                inc     si
                dec     cx

loc_12D6:                               ; CODE XREF: PROCESS_INPUT_V+12↑j
                and     ds:HAYES_STATE, 0F7FFh

locret_12DC:                            ; CODE XREF: PROCESS_INPUT_V+E↑j
                retn
PROCESS_INPUT_V endp


; =============== S U B R O U T I N E =======================================

; Attributes: bp-based frame

DISPATCH_MODEM_ACTION proc near         ; CODE XREF: PROCESS_INPUT_A+12↑p
                                        ; PROCESS_INPUT_A+25↑p ...

arg_0           = word ptr  4
arg_2           = word ptr  6
arg_4           = word ptr  8

                push    bp
                mov     bp, sp
                push    bx
                push    cx
                push    dx
                push    ds
                push    es
                push    si
                push    di
                mov     ax, [bp+arg_4]
                cmp     ax, 11
                ja      short INVALID_ACTION
                shl     ax, 1
                mov     di, offset MODEM_ACTIONS
                add     di, ax
                mov     es, [bp+arg_2]
                mov     si, [bp+arg_0]
                jmp     word ptr [di]
; ---------------------------------------------------------------------------

INVALID_ACTION:                         ; CODE XREF: DISPATCH_MODEM_ACTION+10↑j
                mov     ax, 3
                stc

DISPATCH_MODEM_ACTION_EPILOGUE:         ; CODE XREF: RESET_MODEM+3↓j
                                        ; SET_BREAK_AND_CONNECT_TIMEOUT:loc_1326↓j ...
                pop     di
                pop     si
                pop     es
                pop     ds
                pop     dx
                pop     cx
                pop     bx
                pop     bp
                retn    6
DISPATCH_MODEM_ACTION endp


; =============== S U B R O U T I N E =======================================


RESET_MODEM     proc near               ; DATA XREF: seg000:MODEM_ACTIONS↑o
                call    RESET
                jmp     short DISPATCH_MODEM_ACTION_EPILOGUE
RESET_MODEM     endp


; =============== S U B R O U T I N E =======================================


SET_BREAK_AND_CONNECT_TIMEOUT proc near ; DATA XREF: seg000:03D4↑o
                lods    word ptr es:[si]
                cmp     ax, 0FFFFh
                jz      short loc_131C
                mov     ds:READ_CHAR_TIMEOUT, ax

loc_131C:                               ; CODE XREF: SET_BREAK_AND_CONNECT_TIMEOUT+5↑j
                lods    word ptr es:[si]
                cmp     ax, 0FFFFh
                jz      short loc_1326
                mov     ds:REG_S7, ax   ; Seconds

loc_1326:                               ; CODE XREF: SET_BREAK_AND_CONNECT_TIMEOUT+F↑j
                jmp     short DISPATCH_MODEM_ACTION_EPILOGUE
SET_BREAK_AND_CONNECT_TIMEOUT endp


; =============== S U B R O U T I N E =======================================


MODEM_DISABLE_CARRIER_UNCOND proc near  ; DATA XREF: seg000:03D6↑o
                and     ds:MODEM_STATE_BITS, 111111101101111b
                mov     al, 7
                mov     ah, 0
                call    MODEM_WRITE_REG_CRIT
                mov     al, 2
                call    MODEM_EXEC_COMMAND_CRIT
                test    ds:MODEM_STATE_BITS, MDM_CARRIER_DISABLED
                jz      short EXIT
                mov     ax, 0DFE0h
                mov     es, ax
                assume es:nothing
                mov     es:MODEM_COMMAND_C6, 1

EXIT:                                   ; CODE XREF: MODEM_DISABLE_CARRIER_UNCOND+18↑j
                clc
                jmp     short DISPATCH_MODEM_ACTION_EPILOGUE
MODEM_DISABLE_CARRIER_UNCOND endp


; =============== S U B R O U T I N E =======================================


START_HANDSHAKE_AND_WAIT_CONNECT proc near ; DATA XREF: seg000:03D8↑o
                mov     si, offset MODEM_STATE_BITS
                mov     ah, 10h
                mov     al, 6
                call    MODEM_WRITE_REG_CRIT
                mov     ah, ds:SHADOW_MODEM_OPTIONS.BAUD_RATE_AND_DELAY
                and     ah, 0Fh
                mov     al, 1
                or      ah, 0C0h
                call    MODEM_WRITE_REG_CRIT
                or      word ptr [si], 10000b
                mov     ax, 0DFE0h
                mov     es, ax
                mov     es:MODEM_COMMAND_C6, 0
                mov     bl, ds:byte_6E
                test    word ptr [si], 20h
                jnz     short loc_1387
                mov     al, 8
                call    MODEM_EXEC_COMMAND_CRIT

loc_1387:                               ; CODE XREF: START_HANDSHAKE_AND_WAIT_CONNECT+30↑j
                test    word ptr [si], 20h
                jz      short loc_1392
                cmp     bl, 0
                jz      short MODEM_COMMAND_3?

loc_1392:                               ; CODE XREF: START_HANDSHAKE_AND_WAIT_CONNECT+3B↑j
                cmp     bl, 1
                jnz     short loc_139B
                mov     al, 3
                jmp     short MODEM_EXEC_COMMAND?
; ---------------------------------------------------------------------------

loc_139B:                               ; CODE XREF: START_HANDSHAKE_AND_WAIT_CONNECT+45↑j
                mov     al, 4
                jmp     short MODEM_EXEC_COMMAND?
; ---------------------------------------------------------------------------

MODEM_COMMAND_3?:                       ; CODE XREF: START_HANDSHAKE_AND_WAIT_CONNECT+40↑j
                test    word ptr [si], 40h
                jz      short MODEM_COMMAND_4?
                mov     al, 3
                jmp     short MODEM_EXEC_COMMAND?
; ---------------------------------------------------------------------------

MODEM_COMMAND_4?:                       ; CODE XREF: START_HANDSHAKE_AND_WAIT_CONNECT+53↑j
                mov     al, 4

MODEM_EXEC_COMMAND?:                    ; CODE XREF: START_HANDSHAKE_AND_WAIT_CONNECT+49↑j
                                        ; START_HANDSHAKE_AND_WAIT_CONNECT+4D↑j ...
                call    MODEM_EXEC_COMMAND_CRIT
                mov     ax, ds:REG_S7   ; Seconds
                mov     bx, 1122
                mul     bx
                xor     bx, bx
                mov     ds:word_98, ax
                jnb     short loc_13BE
                inc     bx

loc_13BE:                               ; CODE XREF: START_HANDSHAKE_AND_WAIT_CONNECT+6B↑j
                mov     ds:word_9A, bx
                call    GET_TIME_COUNTER
                add     ds:word_98, ax
                adc     ds:word_9A, bx
                test    cs:MODEM_STATE_BITS, 80h
                jnz     short EXIT

loc_13D6:                               ; CODE XREF: START_HANDSHAKE_AND_WAIT_CONNECT+B6↓j
                                        ; START_HANDSHAKE_AND_WAIT_CONNECT+BE↓j
                test    cs:MODEM_STATE_BITS, 4000h
                jz      short loc_13FD
                or      cs:MODEM_STATE_BITS, 8080h
                cmp     ds:SHADOW_MODEM_OPTIONS.UNK_8, 0FFh
                jz      short loc_13F4
                mov     ah, 80h
                mov     al, 6
                call    MODEM_WRITE_REG_CRIT

loc_13F4:                               ; CODE XREF: START_HANDSHAKE_AND_WAIT_CONNECT+9B↑j
                mov     ah, 4
                mov     al, 7
                call    MODEM_WRITE_REG_CRIT
                jmp     short EXIT
; ---------------------------------------------------------------------------

loc_13FD:                               ; CODE XREF: START_HANDSHAKE_AND_WAIT_CONNECT+8D↑j
                call    GET_TIME_COUNTER
                cmp     bx, ds:word_9A
                ja      short ERROR
                jb      short loc_13D6
                cmp     ax, ds:word_98
                jnb     short ERROR
                jmp     short loc_13D6
; ---------------------------------------------------------------------------

ERROR:                                  ; CODE XREF: START_HANDSHAKE_AND_WAIT_CONNECT+B4↑j
                                        ; START_HANDSHAKE_AND_WAIT_CONNECT+BC↑j
                mov     ax, 0FDh
                stc

EXIT:                                   ; CODE XREF: START_HANDSHAKE_AND_WAIT_CONNECT+84↑j
                                        ; START_HANDSHAKE_AND_WAIT_CONNECT+AB↑j
                jmp     DISPATCH_MODEM_ACTION_EPILOGUE
START_HANDSHAKE_AND_WAIT_CONNECT endp


; =============== S U B R O U T I N E =======================================


GO_OFF_HOOK     proc near               ; DATA XREF: seg000:03DA↑o
                mov     al, 1
                call    MODEM_EXEC_COMMAND_CRIT
                mov     cx, 40000
                mov     al, 1

WAIT_MODEM:                             ; CODE XREF: GO_OFF_HOOK+12↓j
                call    MODEM_READ_STATUS_CRIT
                test    ah, 1
                jnz     short loc_1431
                loop    WAIT_MODEM
                mov     ax, 0FFh        ; Timeout
                stc
                jmp     short DONE
; ---------------------------------------------------------------------------

loc_1431:                               ; CODE XREF: GO_OFF_HOOK+10↑j
                or      ds:MODEM_STATE_BITS, MDM_OFF_HOOK

DONE:                                   ; CODE XREF: GO_OFF_HOOK+18↑j
                jmp     DISPATCH_MODEM_ACTION_EPILOGUE
GO_OFF_HOOK     endp


; =============== S U B R O U T I N E =======================================


GO_ON_HOOK      proc near               ; DATA XREF: seg000:03DC↑o
                mov     ax, 120
                call    ds:DELAY_MS
                mov     ah, 0
                mov     al, 7
                call    MODEM_WRITE_REG_CRIT
                mov     al, 2
                call    MODEM_EXEC_COMMAND_CRIT
                mov     al, 0
                call    MODEM_EXEC_COMMAND_CRIT
                mov     cx, 40000
                mov     al, 1

WAIT_MODEM:                             ; CODE XREF: GO_ON_HOOK+25↓j
                call    MODEM_READ_STATUS_CRIT
                test    ah, 1
                jnz     short READY
                loop    WAIT_MODEM
                mov     ax, 0FFh
                stc
                jmp     short TIMEOUT
; ---------------------------------------------------------------------------

READY:                                  ; CODE XREF: GO_ON_HOOK+23↑j
                mov     ax, 0DFE0h
                mov     es, ax
                mov     es:MODEM_COMMAND_C6, 0
                and     ds:MODEM_STATE_BITS, not 8000h
                mov     cx, ds:REG_S10  ; Time in milliseconds to wait before disconnecting after a remote hangs up.

WAIT_MODEM_2:                           ; CODE XREF: GO_ON_HOOK+49↓j
                mov     ax, 1
                call    ds:DELAY_MS
                loop    WAIT_MODEM_2
                and     ds:MODEM_STATE_BITS, 1010111000001111b
                mov     ah, 8
                mov     al, 6
                call    MODEM_WRITE_REG_CRIT

TIMEOUT:                                ; CODE XREF: GO_ON_HOOK+2B↑j
                jmp     DISPATCH_MODEM_ACTION_EPILOGUE
GO_ON_HOOK      endp


; =============== S U B R O U T I N E =======================================


SET_HANDSHAKE_MODE proc near            ; DATA XREF: seg000:03DE↑o
                lods    byte ptr es:[si]
                cmp     al, 0FFh
                jz      short DONE      ; Keep current value
                cmp     al, 2
                jbe     short SAVE_VALUE
                mov     ax, 0FEh        ; Bad input
                stc
                jmp     short DONE
; ---------------------------------------------------------------------------

SAVE_VALUE:                             ; CODE XREF: SET_HANDSHAKE_MODE+8↑j
                mov     ds:byte_6E, al

DONE:                                   ; CODE XREF: SET_HANDSHAKE_MODE+4↑j
                                        ; SET_HANDSHAKE_MODE+E↑j
                jmp     DISPATCH_MODEM_ACTION_EPILOGUE
SET_HANDSHAKE_MODE endp


; =============== S U B R O U T I N E =======================================


DIAL_NUMBER     proc near               ; DATA XREF: seg000:03E0↑o
                test    ds:MODEM_STATE_BITS, MDM_DIALED
                jnz     short DIAL
                mov     ah, 0
                mov     al, 6
                call    MODEM_WRITE_REG_CRIT
                mov     al, 1
                call    MODEM_EXEC_COMMAND_CRIT
                mov     al, 1

WAIT_MODEM:                             ; CODE XREF: DIAL_NUMBER+1C↓j
                call    MODEM_READ_STATUS_CRIT
                test    ah, 1
                jz      short WAIT_MODEM

DIAL:                                   ; CODE XREF: DIAL_NUMBER+6↑j
                or      ds:MODEM_STATE_BITS, MDM_OFF_HOOK
                and     ds:MODEM_STATE_BITS, not 2000h
                call    PROCESS_PHONE_NUMBER
                or      ds:MODEM_STATE_BITS, MDM_DIALED
                clc
                jmp     DISPATCH_MODEM_ACTION_EPILOGUE
DIAL_NUMBER     endp


; =============== S U B R O U T I N E =======================================


MODEM_DISABLE_CARRIER proc near         ; DATA XREF: seg000:03E2↑o
                or      ds:MODEM_STATE_BITS, MDM_CARRIER_DISABLED
                test    ds:MODEM_STATE_BITS, MDM_HANDSHAKE_ACTIVE
                jnz     short DONE
                mov     ax, 0DFE0h
                mov     es, ax
                mov     es:MODEM_COMMAND_C6, 1

DONE:                                   ; CODE XREF: MODEM_DISABLE_CARRIER+C↑j
                clc
                jmp     DISPATCH_MODEM_ACTION_EPILOGUE
MODEM_DISABLE_CARRIER endp


; =============== S U B R O U T I N E =======================================


MODEM_ENABLE_CARRIER proc near          ; DATA XREF: seg000:03E4↑o
                and     ds:MODEM_STATE_BITS, not MDM_CARRIER_DISABLED
                test    ds:MODEM_STATE_BITS, MDM_HANDSHAKE_ACTIVE
                jnz     short DONE
                mov     ax, 0DFE0h
                mov     es, ax
                mov     es:MODEM_COMMAND_C6, 0

DONE:                                   ; CODE XREF: MODEM_ENABLE_CARRIER+C↑j
                clc
                jmp     DISPATCH_MODEM_ACTION_EPILOGUE
MODEM_ENABLE_CARRIER endp


; =============== S U B R O U T I N E =======================================


MODEM_SET_HANGUP_TIMERS proc near       ; DATA XREF: seg000:03E6↑o
                lods    word ptr es:[si]
                cmp     ax, 0FFFFh
                jz      short SET_S10
                mov     ds:word_5A, ax

SET_S10:                                ; CODE XREF: MODEM_SET_HANGUP_TIMERS+5↑j
                lods    word ptr es:[si]
                cmp     ax, 0FFFFh
                jz      short DONE
                mov     ds:REG_S10, ax  ; Time in milliseconds to wait before disconnecting after a remote hangs up.

DONE:                                   ; CODE XREF: MODEM_SET_HANGUP_TIMERS+F↑j
                jmp     DISPATCH_MODEM_ACTION_EPILOGUE
MODEM_SET_HANGUP_TIMERS endp


; =============== S U B R O U T I N E =======================================


MODEM_SET_VOLUME_FROM_MEM proc near     ; DATA XREF: seg000:MODEM_SET_VOLUME_FROM_MEM_INDIR↑o
                lods    byte ptr es:[si]
                mov     bx, 0DFE0h
                mov     es, bx
                cmp     al, 0
                jnz     short IS_LOUD
                and     es:MODEM_CONTROL_C2, 7Fh
                mov     byte ptr es:44h, 0
                jmp     short loc_1556
; ---------------------------------------------------------------------------

IS_LOUD:                                ; CODE XREF: MODEM_SET_VOLUME_FROM_MEM+9↑j
                or      es:MODEM_CONTROL_C2, 80h
                mov     es:44h, al

loc_1556:                               ; CODE XREF: MODEM_SET_VOLUME_FROM_MEM+17↑j
                jmp     DISPATCH_MODEM_ACTION_EPILOGUE
MODEM_SET_VOLUME_FROM_MEM endp


; =============== S U B R O U T I N E =======================================


PROCESS_PHONE_NUMBER proc near          ; CODE XREF: DIAL_NUMBER+2A↑p
                push    es
                mov     ax, 0DFE0h
                mov     es, ax
                mov     al, es:MODEM_CONTROL_C2
                and     al, 80h
                mov     ds:byte_75, al
                pop     es
                assume es:nothing
                lods    byte ptr es:[si]
                or      al, al
                jz      short DONE
                xor     ch, ch
                mov     cl, al
                mov     ax, ds:REG_S6
                call    ds:DELAY_MS

LOOP_START:                             ; CODE XREF: PROCESS_PHONE_NUMBER:CONTINUE↓j
                lods    byte ptr es:[si]
                cmp     al, '.'
                jnz     short CHECK_SEPARATORS_AND_P
                mov     ax, ds:REG_S8   ; Milliseconds
                call    ds:DELAY_MS
                jmp     short CONTINUE
; ---------------------------------------------------------------------------

CHECK_SEPARATORS_AND_P:                 ; CODE XREF: PROCESS_PHONE_NUMBER+25↑j
                cmp     al, '('
                jz      short CONTINUE
                cmp     al, '-'
                jz      short CONTINUE
                cmp     al, ')'
                jz      short CONTINUE
                cmp     al, ' '
                jz      short CONTINUE
                cmp     al, 0
                jz      short CONTINUE
                cmp     al, 'P'
                jnz     short CHECK_T
                and     ds:STATE_BITS, 1110111111111111b
                jmp     short CONTINUE
; ---------------------------------------------------------------------------

CHECK_T:                                ; CODE XREF: PROCESS_PHONE_NUMBER+46↑j
                cmp     al, 'T'
                jnz     short CHECK_NSIGN
                or      ds:STATE_BITS, 1000h
                jmp     short CONTINUE
; ---------------------------------------------------------------------------

CHECK_NSIGN:                            ; CODE XREF: PROCESS_PHONE_NUMBER+52↑j
                cmp     al, '#'
                jnz     short CHECK_STAR
                mov     al, 0Ah
                jmp     short EMIT_DIGIT
; ---------------------------------------------------------------------------

CHECK_STAR:                             ; CODE XREF: PROCESS_PHONE_NUMBER+5E↑j
                cmp     al, '*'
                jnz     short CHECK_CARET
                mov     al, 0Bh
                jmp     short EMIT_DIGIT
; ---------------------------------------------------------------------------

CHECK_CARET:                            ; CODE XREF: PROCESS_PHONE_NUMBER+66↑j
                cmp     al, '^'
                jnz     short CONVERT_DIGIT_TO_VALUE
                mov     ax, 200
                call    ds:DELAY_MS
                mov     ax, ds:REG_S6
                call    ds:DELAY_MS
                mov     al, 1
                call    MODEM_READ_STATUS_CRIT
                test    ah, 4
                jz      short ERROR
                jmp     short CONTINUE
; ---------------------------------------------------------------------------

CONVERT_DIGIT_TO_VALUE:                 ; CODE XREF: PROCESS_PHONE_NUMBER+6E↑j
                sub     al, '0'

EMIT_DIGIT:                             ; CODE XREF: PROCESS_PHONE_NUMBER+62↑j
                                        ; PROCESS_PHONE_NUMBER+6A↑j
                call    EMIT_DIGIT
                jb      short EXIT

CONTINUE:                               ; CODE XREF: PROCESS_PHONE_NUMBER+2E↑j
                                        ; PROCESS_PHONE_NUMBER+32↑j ...
                loop    LOOP_START

DONE:                                   ; CODE XREF: PROCESS_PHONE_NUMBER+14↑j
                clc
                jmp     short EXIT
; ---------------------------------------------------------------------------

ERROR:                                  ; CODE XREF: PROCESS_PHONE_NUMBER+86↑j
                mov     ax, 0FCh
                stc

EXIT:                                   ; CODE XREF: PROCESS_PHONE_NUMBER+8F↑j
                                        ; PROCESS_PHONE_NUMBER+94↑j
                retn
PROCESS_PHONE_NUMBER endp


; =============== S U B R O U T I N E =======================================


EMIT_DIGIT      proc near               ; CODE XREF: PROCESS_PHONE_NUMBER:EMIT_DIGIT↑p
                test    ds:STATE_BITS, 1000h
                jz      short CHECK_PULSES
                cmp     al, 11
                ja      short BAD_INPUT
                push    si              ; Touch tone
                mov     si, offset DTMF_TABLE
                xchg    ax, bx
                xor     bh, bh
                mov     dx, ds:REG_S11  ; Speed of the touch-tone dialer. In milliseconds.
                mov     ax, dx
                call    ds:DELAY_MS
                push    es
                mov     ax, 0DFE0h      ; Emit tone
                mov     es, ax
                assume es:nothing
                mov     al, [bx+si]
                or      al, ds:byte_75
                mov     es:MODEM_CONTROL_C2, al
                mov     ax, dx
                call    ds:DELAY_MS
                mov     al, 7Fh         ; Disable tone
                or      al, ds:byte_75
                mov     es:MODEM_CONTROL_C2, al
                pop     es
                assume es:nothing
                pop     si
                jmp     short DONE
; ---------------------------------------------------------------------------

CHECK_PULSES:                           ; CODE XREF: EMIT_DIGIT+6↑j
                cmp     al, 9
                ja      short BAD_INPUT
                cmp     al, 0
                jnz     short EMIT_PULSE
                mov     al, 10          ; "0" is 10 pulses

EMIT_PULSE:                             ; CODE XREF: EMIT_DIGIT+47↑j
                or      al, 1000000b
                call    MODEM_EXEC_COMMAND_CRIT
                push    cx
                mov     cx, 5000

WAIT_MODEM:                             ; CODE XREF: EMIT_DIGIT+63↓j
                mov     ax, 1
                call    ds:DELAY_MS
                mov     al, 1
                call    MODEM_READ_STATUS_CRIT
                test    ah, 1
                loope   WAIT_MODEM
                pop     cx
                jz      short TIMEOUT

DONE:                                   ; CODE XREF: EMIT_DIGIT+3F↑j
                clc
                jmp     short EXIT
; ---------------------------------------------------------------------------

BAD_INPUT:                              ; CODE XREF: EMIT_DIGIT+A↑j
                                        ; EMIT_DIGIT+43↑j
                mov     ax, 0FBh
                jmp     short ERROR
; ---------------------------------------------------------------------------

TIMEOUT:                                ; CODE XREF: EMIT_DIGIT+66↑j
                mov     ax, 0FCh

ERROR:                                  ; CODE XREF: EMIT_DIGIT+6E↑j
                stc

EXIT:                                   ; CODE XREF: EMIT_DIGIT+69↑j
                retn
EMIT_DIGIT      endp


; =============== S U B R O U T I N E =======================================


TEST_FLAGS_DO_NUMBERS? proc near        ; CODE XREF: AT_COMMAND_PARSER:loc_18BC↓p
                pushf
                mov     di, offset HAYES_STATE
                jnb     short loc_1673
                or      word ptr [di], 1000000000b

loc_1673:                               ; CODE XREF: TEST_FLAGS_DO_NUMBERS?+4↑j
                test    word ptr [di], 1000000000000b
                jnz     short loc_16C6
                test    word ptr [di], 1000000000b
                jz      short loc_1684
                mov     si, offset a4   ; "4\r"
                jmp     short loc_16B1
; ---------------------------------------------------------------------------

loc_1684:                               ; CODE XREF: TEST_FLAGS_DO_NUMBERS?+14↑j
                test    word ptr [di], 10000000000000b
                jnz     short loc_168F
                mov     si, offset a0   ; "0\r"
                jmp     short loc_16B1
; ---------------------------------------------------------------------------

loc_168F:                               ; CODE XREF: TEST_FLAGS_DO_NUMBERS?+1F↑j
                test    ds:MODEM_STATE_BITS, 80h
                jnz     short loc_169C
                mov     si, offset a3   ; "3\r"
                jmp     short loc_16B1
; ---------------------------------------------------------------------------

loc_169C:                               ; CODE XREF: TEST_FLAGS_DO_NUMBERS?+2C↑j
                test    ds:SHADOW_MODEM_OPTIONS.BAUD_RATE_AND_DELAY, 4
                jnz     short loc_16AE
                test    word ptr [di], 400h
                jz      short loc_16AE
                mov     si, offset a5   ; "5\r"
                jmp     short loc_16B1
; ---------------------------------------------------------------------------

loc_16AE:                               ; CODE XREF: TEST_FLAGS_DO_NUMBERS?+38↑j
                                        ; TEST_FLAGS_DO_NUMBERS?+3E↑j
                mov     si, offset a1   ; "1\r"

loc_16B1:                               ; CODE XREF: TEST_FLAGS_DO_NUMBERS?+19↑j
                                        ; TEST_FLAGS_DO_NUMBERS?+24↑j ...
                test    word ptr [di], 100000000000b
                jz      short loc_16BE
                xor     ax, ax
                mov     al, [si]
                inc     si
                add     si, ax

loc_16BE:                               ; CODE XREF: TEST_FLAGS_DO_NUMBERS?+4C↑j
                xor     cx, cx
                mov     cl, [si]
                inc     si
                call    RX_BUFFER_WRITE_BYTES

loc_16C6:                               ; CODE XREF: TEST_FLAGS_DO_NUMBERS?+E↑j
                and     ds:HAYES_STATE, 1111110111111111b
                popf
                retn
TEST_FLAGS_DO_NUMBERS? endp


; =============== S U B R O U T I N E =======================================


DETECT_HAYES_ESCAPE_SEQUENCE proc near  ; CODE XREF: COM2_OUTPUT_IMPL+17↑p
                push    cx
                push    di
                mov     di, si
                mov     al, ds:REG_S2
                repe scasb
                pop     di
                pop     cx
                jnz     short DONE
                or      word ptr [di], MDM_RING_SEEN
                mov     al, ds:ESCAPE_CHAR_COUNT
                add     al, cl
                cmp     al, 3
                ja      short DONE
                mov     ds:ESCAPE_CHAR_COUNT, al
                jb      short DONE
                and     word ptr [di], not MDM_RING_SEEN
                xor     al, al
                mov     ds:ESCAPE_CHAR_COUNT, al
                stc
                jmp     short EXIT
; ---------------------------------------------------------------------------

DONE:                                   ; CODE XREF: DETECT_HAYES_ESCAPE_SEQUENCE+B↑j
                                        ; DETECT_HAYES_ESCAPE_SEQUENCE+18↑j ...
                clc

EXIT:                                   ; CODE XREF: DETECT_HAYES_ESCAPE_SEQUENCE+29↑j
                retn
DETECT_HAYES_ESCAPE_SEQUENCE endp


; =============== S U B R O U T I N E =======================================


UPDATE_LAST_COM2_OUTPUT_TIMESTAMP proc near ; CODE XREF: COM2_OUTPUT_IMPL:DONE↑p
                call    GET_TIME_COUNTER
                mov     word ptr ds:LAST_COM2_OUTPUT_TIME, ax
                mov     word ptr ds:LAST_COM2_OUTPUT_TIME+2, bx
                retn
UPDATE_LAST_COM2_OUTPUT_TIMESTAMP endp


; =============== S U B R O U T I N E =======================================


GET_TIME_COUNTER proc near              ; CODE XREF: RESET+C5↑p
                                        ; START_HANDSHAKE_AND_WAIT_CONNECT+72↑p ...
                push    si
                push    ds
                push    es
                xor     ax, ax
                mov     es, ax
                assume es:seg000
                lds     si, es:dword_200
                add     si, 48h
                cli
                mov     ax, [si]
                mov     bx, [si+2]
                sti
                pop     es
                assume es:nothing
                pop     ds
                pop     si
                retn
GET_TIME_COUNTER endp


; =============== S U B R O U T I N E =======================================


ESCAPE_GUARD_WHAT?? proc near           ; CODE XREF: COM2_OUTPUT_IMPL+C↑p
                call    GET_TIME_COUNTER
                sub     ax, word ptr ds:LAST_COM2_OUTPUT_TIME
                sbb     bx, word ptr ds:LAST_COM2_OUTPUT_TIME+2
                cmp     bx, ds:word_96
                jb      short loc_1739
                ja      short ERROR
                cmp     ax, ds:REG_S12  ; Milliseconds
                jnb     short ERROR

loc_1739:                               ; CODE XREF: ESCAPE_GUARD_WHAT??+F↑j
                test    word ptr [di], 2000h
                jnz     short ERROR
                xor     al, al
                mov     ds:ESCAPE_CHAR_COUNT, al
                clc
                jmp     short EXIT
; ---------------------------------------------------------------------------

ERROR:                                  ; CODE XREF: ESCAPE_GUARD_WHAT??+11↑j
                                        ; ESCAPE_GUARD_WHAT??+17↑j ...
                stc

EXIT:                                   ; CODE XREF: ESCAPE_GUARD_WHAT??+25↑j
                retn
ESCAPE_GUARD_WHAT?? endp


; =============== S U B R O U T I N E =======================================


RX_BUFFER_WRITE_BYTES proc near         ; CODE XREF: COM2_OUTPUT_IMPL+6F↑p
                                        ; seg000:11B5↑p ...
                push    es
                push    di
                push    si
                push    bx
                push    dx
                les     bx, dword ptr cs:RXBUF_FARPTR_OFFSET
                mov     di, cs:RXBUF_WRITE_CURSOR
                mov     ax, cs:RXBUF_READ_CURSOR
                dec     ax
                mov     dx, cs:RXBUF_SIZE
                dec     dx

loc_1763:                               ; CODE XREF: RX_BUFFER_WRITE_BYTES+38↓j
                cmp     di, ax
                jz      short loc_1779
                cmp     di, dx
                jnz     short loc_1778
                cmp     cs:RXBUF_READ_CURSOR, 0
                jz      short loc_1779
                mov     di, 0
                jmp     short loc_1779
; ---------------------------------------------------------------------------

loc_1778:                               ; CODE XREF: RX_BUFFER_WRITE_BYTES+20↑j
                inc     di

loc_1779:                               ; CODE XREF: RX_BUFFER_WRITE_BYTES+1C↑j
                                        ; RX_BUFFER_WRITE_BYTES+28↑j ...
                push    dx
                mov     dl, [si]
                inc     si
                mov     es:[bx+di], dl
                pop     dx
                loop    loc_1763
                mov     cs:RXBUF_WRITE_CURSOR, di
                pop     dx
                pop     bx
                pop     si
                pop     di
                pop     es
                retn
RX_BUFFER_WRITE_BYTES endp


; =============== S U B R O U T I N E =======================================


sub_178E        proc near               ; CODE XREF: RESET+DF↑p
                push    bx
                push    ds
                push    cs
                push    di
                mov     di, offset a0   ; "0\r"
                mov     cx, 6
                push    cs
                pop     ds
                assume ds:seg000
                xor     bx, bx

loc_179C:                               ; CODE XREF: sub_178E+25↓j
                mov     [di+2], ah
                mov     bl, [di+3]
                mov     [di+4], ah
                mov     [di+5], al
                mov     [bx+di+2], ah
                mov     [bx+di+3], al
                add     di, bx
                add     di, 4
                loop    loc_179C
                mov     di, (offset a000+1)
                mov     [di], ah
                mov     [di+1], al
                mov     [di+5], ah
                mov     [di+6], al
                pop     di
                pop     cx
                pop     ds
                assume ds:nothing
                pop     bx
                retn
sub_178E        endp


; =============== S U B R O U T I N E =======================================


MODEM_SET_VOLUME proc near              ; CODE XREF: RESET+2E↑p
                                        ; INT82H_OP_SET_VOLUME+15↑p ...
                push    es
                push    bx
                mov     bx, 0DFE0h
                mov     es, bx
                assume es:nothing
                cmp     al, 0
                jnz     short IS_LOUD
                and     es:MODEM_CONTROL_C2, 01111111b
                jmp     short SET_VOLUME
; ---------------------------------------------------------------------------

IS_LOUD:                                ; CODE XREF: MODEM_SET_VOLUME+9↑j
                or      es:MODEM_CONTROL_C2, 10000000b

SET_VOLUME:                             ; CODE XREF: MODEM_SET_VOLUME+11↑j
                mov     es:44h, al
                pop     bx
                pop     es
                assume es:nothing
                retn
MODEM_SET_VOLUME endp


; =============== S U B R O U T I N E =======================================


AT_COMMAND_PARSER proc near             ; CODE XREF: COM2_OUTPUT_IMPL:COMMAND_MODE↑p
                xor     bh, bh
                mov     bl, ds:AT_COMMAND_BUF_LEN
                mov     di, offset AT_COMMAND_BUF

LOOP_START:                             ; CODE XREF: AT_COMMAND_PARSER:CONTINUE↓j
                lods    byte ptr es:[si]
                test    ds:HAYES_STATE, 100h
                jz      short loc_180D
                push    ax
                push    ds
                push    cx
                dec     si
                mov     ax, es
                mov     ds, ax
                mov     cx, 1
                call    RX_BUFFER_WRITE_BYTES
                inc     si
                pop     cx
                pop     ds
                pop     ax

loc_180D:                               ; CODE XREF: AT_COMMAND_PARSER+11↑j
                test    ds:STATE_BITS, 800h
                jz      short loc_182D
                cmp     al, ds:REG_S3   ; Value of the carriage return.
                jz      short UNEXPECTED_CARRIAGE_RETURN_FOUND
                jmp     LOOP_END
; ---------------------------------------------------------------------------

UNEXPECTED_CARRIAGE_RETURN_FOUND:       ; CODE XREF: AT_COMMAND_PARSER+31↑j
                and     ds:STATE_BITS, 0F7FFh
                xor     bl, bl
                mov     ds:AT_COMMAND_BUF_LEN, bl
                jmp     ERROR
; ---------------------------------------------------------------------------

loc_182D:                               ; CODE XREF: AT_COMMAND_PARSER+2B↑j
                test    ds:STATE_BITS, 400h
                jz      short loc_183E
                or      ds:STATE_BITS, 100h
                jmp     short LOOP_END
; ---------------------------------------------------------------------------
                align 2

loc_183E:                               ; CODE XREF: AT_COMMAND_PARSER+4B↑j
                cmp     al, ' '
                jz      short LOOP_END
                cmp     al, ds:REG_S4
                jz      short LOOP_END
                call    CONVERT_CHAR_TO_UPPERCASE
                mov     [bx+di], al
                inc     bl
                cmp     bl, '('
                jbe     short loc_185C
                or      ds:STATE_BITS, 800h
                jmp     short LOOP_END
; ---------------------------------------------------------------------------

loc_185C:                               ; CODE XREF: AT_COMMAND_PARSER+6A↑j
                mov     ds:AT_COMMAND_BUF_LEN, bl
                test    ds:STATE_BITS, 200h
                jz      short CHECK_A
                cmp     al, ds:REG_S3   ; Value of the carriage return.
                jnz     short LOOP_END
                jmp     short CARRIAGE_RETURN_FOUND
; ---------------------------------------------------------------------------

CHECK_A:                                ; CODE XREF: AT_COMMAND_PARSER+7E↑j
                cmp     bl, 2
                jnz     short LOOP_END
                cmp     byte ptr [di], 'A'
                jz      short CHECK_T
                or      ds:STATE_BITS, 800h
                jmp     short LOOP_END
; ---------------------------------------------------------------------------

CHECK_T:                                ; CODE XREF: AT_COMMAND_PARSER+90↑j
                cmp     al, 'T'
                jnz     short CHECK_SLASH
                or      ds:STATE_BITS, 200h
                xor     bl, bl
                mov     ds:byte_33A, bl
                mov     ds:AT_COMMAND_BUF_LEN, bl
                jmp     short LOOP_END
; ---------------------------------------------------------------------------

CHECK_SLASH:                            ; CODE XREF: AT_COMMAND_PARSER+9C↑j
                cmp     al, '/'
                jz      short SLASH_FOUND
                or      ds:STATE_BITS, 800h

LOOP_END:                               ; CODE XREF: AT_COMMAND_PARSER+33↑j
                                        ; AT_COMMAND_PARSER+53↑j ...
                loop    CONTINUE
                clc
                jmp     short EXIT
; ---------------------------------------------------------------------------

CONTINUE:                               ; CODE XREF: AT_COMMAND_PARSER:LOOP_END↑j
                jmp     LOOP_START
; ---------------------------------------------------------------------------

SLASH_FOUND:                            ; CODE XREF: AT_COMMAND_PARSER+B2↑j
                or      ds:STATE_BITS, 80h

CARRIAGE_RETURN_FOUND:                  ; CODE XREF: AT_COMMAND_PARSER+86↑j
                or      ds:STATE_BITS, 400h
                call    PROCESS_INPUT
                jmp     short loc_18BC
; ---------------------------------------------------------------------------

ERROR:                                  ; CODE XREF: AT_COMMAND_PARSER+42↑j
                stc

loc_18BC:                               ; CODE XREF: AT_COMMAND_PARSER+D1↑j
                call    TEST_FLAGS_DO_NUMBERS?
                and     ds:STATE_BITS, not 600h

EXIT:                                   ; CODE XREF: AT_COMMAND_PARSER+BD↑j
                retn
AT_COMMAND_PARSER endp


; =============== S U B R O U T I N E =======================================


PROCESS_INPUT   proc near               ; CODE XREF: AT_COMMAND_PARSER+CE↑p
                test    ds:STATE_BITS, 80h
                jnz     short loc_18EC
                mov     si, 2E9h
                mov     di, 311h
                push    si
                push    es
                mov     ax, ds
                mov     es, ax
                xor     ch, ch
                mov     cl, ds:AT_COMMAND_BUF_LEN
                mov     ds:byte_33A, cl
                push    cx
                rep movsb
                pop     cx
                pop     es
                pop     si
                jmp     short CHECK_CX_BEFORE_LOOP
; ---------------------------------------------------------------------------

loc_18EC:                               ; CODE XREF: PROCESS_INPUT+6↑j
                mov     si, 311h
                xor     ch, ch
                mov     cl, ds:byte_33A

CHECK_CX_BEFORE_LOOP:                   ; CODE XREF: PROCESS_INPUT+24↑j
                cmp     cx, 0
                jnz     short LOOP_START
                jmp     PROCESS_INPUT_DONE
; ---------------------------------------------------------------------------

LOOP_START:                             ; CODE XREF: PROCESS_INPUT+32↑j
                                        ; PROCESS_INPUT:REPEAT_LOOP↓j
                and     ds:HAYES_STATE, not 2000h
                lodsb
                cmp     al, 'A'
                jnz     short CHECK_C
                or      ds:HAYES_STATE, 2000h
                call    PROCESS_INPUT_A
                and     ds:STATE_BITS, not 8000h
                jmp     PROCESS_INPUT_DONE
; ---------------------------------------------------------------------------

CHECK_C:                                ; CODE XREF: PROCESS_INPUT+40↑j
                cmp     al, 'C'
                jnz     short CHECK_D
                call    PROCESS_INPUT_C
                jmp     LOOP_END
; ---------------------------------------------------------------------------

CHECK_D:                                ; CODE XREF: PROCESS_INPUT+56↑j
                cmp     al, 'D'
                jnz     short CHECK_E
                or      ds:HAYES_STATE, 2000h
                call    PROCESS_INPUT_D
                test    ds:STATE_BITS, 8000h
                jnz     short LOOP_END
                jmp     short PROCESS_INPUT_DONE
; ---------------------------------------------------------------------------

CHECK_E:                                ; CODE XREF: PROCESS_INPUT+60↑j
                cmp     al, 'E'
                jnz     short CHECK_F
                call    PROCESS_INPUT_E
                jmp     short LOOP_END
; ---------------------------------------------------------------------------

CHECK_F:                                ; CODE XREF: PROCESS_INPUT+77↑j
                cmp     al, 'F'
                jnz     short CHECK_H
                call    PROCESS_INPUT_F
                jmp     short LOOP_END
; ---------------------------------------------------------------------------

CHECK_H:                                ; CODE XREF: PROCESS_INPUT+80↑j
                cmp     al, 'H'
                jnz     short CHECK_M
                call    PROCESS_INPUT_H
                jmp     short LOOP_END
; ---------------------------------------------------------------------------

CHECK_M:                                ; CODE XREF: PROCESS_INPUT+89↑j
                cmp     al, 'M'
                jnz     short CHECK_O
                call    PROCESS_INPUT_M
                jmp     short LOOP_END
; ---------------------------------------------------------------------------

CHECK_O:                                ; CODE XREF: PROCESS_INPUT+92↑j
                cmp     al, 'O'
                jnz     short CHECK_P
                call    PROCESS_INPUT_O
                jmp     short PROCESS_INPUT_DONE
; ---------------------------------------------------------------------------
                nop

CHECK_P:                                ; CODE XREF: PROCESS_INPUT+9B↑j
                cmp     al, 'P'
                jnz     short CHECK_Q
                call    PROCESS_INPUT_P
                jmp     short LOOP_END
; ---------------------------------------------------------------------------

CHECK_Q:                                ; CODE XREF: PROCESS_INPUT+A5↑j
                cmp     al, 'Q'
                jnz     short CHECK_S
                call    PROCESS_INPUT_Q
                jmp     short LOOP_END
; ---------------------------------------------------------------------------

CHECK_S:                                ; CODE XREF: PROCESS_INPUT+AE↑j
                cmp     al, 'S'
                jz      short HANDLE_S
                cmp     al, '?'
                jz      short HANDLE_S
                cmp     al, '='
                jnz     short CHECK_T

HANDLE_S:                               ; CODE XREF: PROCESS_INPUT+B7↑j
                                        ; PROCESS_INPUT+BB↑j
                call    PROCESS_INPUT_S
                jmp     short LOOP_END
; ---------------------------------------------------------------------------

CHECK_T:                                ; CODE XREF: PROCESS_INPUT+BF↑j
                cmp     al, 'T'
                jnz     short CHECK_V
                call    PROCESS_INPUT_T
                jmp     short LOOP_END
; ---------------------------------------------------------------------------

CHECK_V:                                ; CODE XREF: PROCESS_INPUT+C8↑j
                cmp     al, 'V'
                jnz     short CHECK_X
                call    PROCESS_INPUT_V
                jmp     short LOOP_END
; ---------------------------------------------------------------------------

CHECK_X:                                ; CODE XREF: PROCESS_INPUT+D1↑j
                cmp     al, 'X'
                jnz     short CHECK_Z
                call    PROCESS_INPUT_X
                jmp     short LOOP_END
; ---------------------------------------------------------------------------

CHECK_Z:                                ; CODE XREF: PROCESS_INPUT+DA↑j
                cmp     al, 'Z'
                jnz     short LOOP_END
                call    PROCESS_INPUT_Z

LOOP_END:                               ; CODE XREF: PROCESS_INPUT+5B↑j
                                        ; PROCESS_INPUT+71↑j ...
                loop    REPEAT_LOOP
                jmp     short PROCESS_INPUT_DONE
; ---------------------------------------------------------------------------

REPEAT_LOOP:                            ; CODE XREF: PROCESS_INPUT:LOOP_END↑j
                jmp     LOOP_START
; ---------------------------------------------------------------------------

PROCESS_INPUT_DONE:                     ; CODE XREF: PROCESS_INPUT+34↑j
                                        ; PROCESS_INPUT+51↑j ...
                clc
                mov     bl, 0
                mov     ds:AT_COMMAND_BUF_LEN, bl
                retn
PROCESS_INPUT   endp


; =============== S U B R O U T I N E =======================================


COM2_REQ_INIT   proc far                ; DATA XREF: seg000:COM2_INT_PROC_TABLE↑o
                                        ; COM2_REQ_INIT↓o
                mov     ax, offset COM2_REQ_INIT
                lds     bx, cs:DevRequest_0
                mov     [bx+0Eh], ax
                mov     ax, cs
                mov     [bx+10h], ax
                mov     ax, 0
                mov     es, ax
                assume es:seg000
                lds     si, es:dword_200
                push    cs
                pop     es
                push    si
                mov     di, offset REG_INT_HANDLER
                mov     cx, 8
                add     si, 24h ; '$'
                rep movsw
                pop     si
                push    si
                mov     di, offset DELAY_MS
                mov     cx, 2
                add     si, 50h ; 'P'
                rep movsw
                pop     si
                add     si, offset READ_CHAR_TIMEOUT
                mov     ax, cs
                mov     [si+2], ax
                mov     [si+6], ax
                mov     ax, 0
                mov     [si], ax
                mov     ax, 12h
                mov     [si+4], ax
                mov     ax, 0DFE0h      ; Checking modem existence
                mov     es, ax
                assume es:nothing
                mov     es:MODEM_CONTROL_C2, 55h ; 'U'
                mov     cx, 100h

INIT_BUSY_LOOP_1:                       ; CODE XREF: COM2_REQ_INIT:INIT_BUSY_LOOP_1↓j
                loop    INIT_BUSY_LOOP_1
                mov     al, es:MODEM_CONTROL_C2
                cmp     al, 55h ; 'U'
                jnz     short MODEM_NOT_FOUND
                mov     es:MODEM_CONTROL_C2, 0AAh
                mov     cx, 100h

INIT_BUSY_LOOP_2:                       ; CODE XREF: COM2_REQ_INIT:INIT_BUSY_LOOP_2↓j
                loop    INIT_BUSY_LOOP_2
                mov     al, es:MODEM_CONTROL_C2
                cmp     al, 0AAh
                jnz     short MODEM_NOT_FOUND
                call    REG_INTERRUPT_82H
                mov     ax, 4
                push    ax
                push    cs
                mov     ax, offset MODEM_INT_HANDLER
                push    ax
                call    cs:REG_INT_HANDLER
                push    cs
                pop     ds
                assume ds:seg000
                call    RESET
                jmp     COM2_REQ_NOTAPP
; ---------------------------------------------------------------------------

MODEM_NOT_FOUND:                        ; CODE XREF: COM2_REQ_INIT+62↑j
                                        ; COM2_REQ_INIT+75↑j
                xor     ah, ah
                mov     al, 0FFh
                mov     cs:REENT_FLAG?, al
                mov     al, 1
                jmp     INT_RET_ERROR
COM2_REQ_INIT   endp

seg000          ends


                end
