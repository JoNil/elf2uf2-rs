# Port of elf2uf2 to rust

```bash
cargo install elf2uf2-rs
```

## Options
-d automatic deployment to a mounted pico.
-s open the pico as a serial device after deploy and print serial output.
-t send termination message to the device if ctrl+c is pressed. Can be used on the device to trigger a reboot into programming mode with a call to reset_to_usb_boot(0, 0)

Original at https://github.com/raspberrypi/pico-sdk/tree/master/tools/elf2uf2