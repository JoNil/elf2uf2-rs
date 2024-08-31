# Port of elf2uf2 to rust

```bash
cargo install elf2uf2-rs
```

## Options
* -d automatic deployment to a mounted pico.
* -c on ChromeOS (Crostini) mounts the RPI-RP2 drive on /mnt/chromeos/removable/RPI-RP2.
* -s open the pico as a serial device after deploy and print serial output.

Original at https://github.com/raspberrypi/pico-sdk/tree/master/tools/elf2uf2