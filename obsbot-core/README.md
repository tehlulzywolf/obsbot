# obsbot-core

Rust control library and panel for the OBSBOT Tiny 2 Lite (`3564:fef9`, product type 3).

Vendor commands travel on the UVC extension unit GUID `{9a1e7291-6843-4683-6d92-39bc7906ee49}`, unit 2, through `UVCIOC_CTRL_QUERY`. Zoom, pan, and tilt use the standard UVC controls. The camera has no HID interface, so there is no hidraw path for this model.

The panel is `examples/panel.rs`. How to build, attach the camera, and run it is in the [repository README](../README.md). Protocol notes that match the current panel are in `../obsbot-center/analysis/HANDOFF.md`.

## Build and test

```sh
cargo test --offline
cargo build --offline --example panel --features panel
cargo run --offline --example panel --features panel
```

`cargo test` needs no camera and no network. The panel feature enables egui.

## Features

| Feature | What it adds |
|---|---|
| `uvc` | Linux userspace UVC transport (`nusb`) |
| `codec` | JPEG decode for the preview |
| `stream` | Capture on top of `uvc` and `codec` |
| `panel` | The egui window |

The panel binary uses the kernel V4L2 path in `src/linux_kernel.rs`.
