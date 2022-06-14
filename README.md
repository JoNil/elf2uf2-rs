# Port of elf2uf2 to rust

## Use as a Binary
```bash
cargo install elf2uf2-rs
```

## Use as a Library
This can also be imported into your cargo managed project as a library.
See the documentation for more information on that.

## Options
-d automatic deployment to a mounted pico.
-s open the pico as a serial device after deploy and print serial output.

Derived from original at https://github.com/raspberrypi/pico-sdk/tree/master/tools/elf2uf2
