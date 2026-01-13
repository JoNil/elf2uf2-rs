# elf2uf2-rs

A tool for converting ELF binaries into UF2 format and flashing them to a microcontroller (RP2040, RP2350, etc.) and other supported boards.

```bash
cargo install elf2uf2-rs
```

## Options

These are the following options for elf2uf2-rs:

```
Usage: elf2uf2-rs [OPTIONS] [COMMAND]

Commands:
  convert  Convert ELF to UF2 file on disk
  deploy   Deploy ELF directly to a connected board
  help     Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose <VERBOSE>  Set the logging verbosity [default: info] [possible values: off, error, warn, info, debug, trace]
  -h, --help               Print help
  -V, --version            Print version
```

### Deploying

These are the following options for deploying to a microcontroller with elf2uf2-rs:

```
Usage: elf2uf2-rs deploy [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Input ELF file

Options:
  -f, --family <FAMILY>    Select family short name for UF2 [default: rp2040] [possible values: rp2040, rp2xxx-absolute, rp2xxx-data, rp2350-arm-s, rp2350-riscv, rp2350-arm-ns]
  -v, --verbose <VERBOSE>  Set the logging verbosity [default: info] [possible values: off, error, warn, info, debug, trace]
  -s, --serial             Connect to serial after deploy
  -t, --term               Send termination message on Ctrl+C
  -h, --help               Print help (see more with '--help')
```

### Deploy for any project

You can deploy to a microcontroller with elf2uf2-rs by running the following command:

```bash
elf2uf2-rs deploy --family rp2040 firmware.elf
```

## Usage

To make your Rust project automatically flash the microcontroller whenever you run `cargo run`, add this to your `.cargo/config.toml`.

```toml
[target.'cfg(all(target_arch = "arm", target_os = "none"))']
runner = "elf2uf2-rs deploy -t -s"
# runner = "elf2uf2-rs deploy -t -s --family rp2350-arm-s" # Pico 2 / Cortex-M23/M33

[build]
# target = "thumbv6m-none-eabi" # Pico 1 / Cortex-M0/M0+
target = "thumbv8m.main-none-eabihf" # Pico 2 / Cortex-M23/M33

[env]
DEFMT_LOG = "debug"
```

## Credits

Based upon Raspberry Pi's [elf2uf2](https://github.com/raspberrypi/pico-sdk/tree/master/tools/elf2uf2)
