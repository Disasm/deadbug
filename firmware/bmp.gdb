set target-async on
set confirm off
set history save
set mem inaccessible-by-default off
tar ext /dev/serial/by-id/usb-Black_Sphere_Technologies_Black_Magic_Probe__STLINK____Firmware_v1.6.1-255-gdcd20ef__D6D3ACD0-if00
mon version
mon swdp_scan
att 1

set print asm-demangle on
set backtrace limit 32
break core::panicking::panic
break core::panicking::panic_fmt
#break core::panicking::panic_impl
break core::slice::slice_index_len_fail
load
run
