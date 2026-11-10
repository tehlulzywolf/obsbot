# OBSBOT Center for Linux

A Linux port of the OBSBOT Center panel for the Tiny 2 Lite (`3564:fef9`). It runs on Ubuntu. The window is a native egui app: it previews the camera and drives the gimbal, zoom, image controls, and the AI tracking modes that have been checked on the hardware.

The camera on this project is product type 3 (Tail2Lite). Vendor commands go out on the UVC extension unit through the kernel's `uvcvideo` driver, not over HID. The Tiny 2 Lite does not expose a HID interface.

![Live preview and the Image tab](docs/images/panel-preview.png)

![The More tab](docs/images/panel-more.png)

## What the panel does

Checked on a live Tiny 2 Lite under Ubuntu 26.04:

- 720p30 MJPEG preview.
- Gimbal pad and speed. Pan moves in 1° steps. The pad sends absolute pan and tilt.
- Manual zoom through the standard UVC zoom control.
- Human tracking. The gimbal follows a person.
- Hand tracking. The Hand button opens the menu. On and Off are the switch. The menu closes when you click the picture. The Hand button stays lit while tracking is on.
- Yaw reverse. With the switch on, dragging the pad left pans the opposite way. Tilt is unchanged.
- Image controls the camera reports, including focus, exposure, white balance, and the sharpness and backlight sliders.
- Sleep and wake, auto-sleep, the status light, and the More-tab audio and sleep-background controls. Those were exercised and the USB link stayed up.

Present, with a limit you should know about:

- Group is on the bar. It was not retested after human tracking started following.
- Left and Right in the hand menu do not pick a hand. The firmware command that turns hand tracking on follows either hand. The pills only show which one is selected.
- Tracking Range is visible and does not send a command. The boundary pack is not wired.
- Orientation accepts values 0 through 3 and the preview stays upright.
- Preset goto uses the legacy command. Save, Delete, and Rename are on the Image tab and were not used.
- Beauty is drawn as a control. The effect in the official app runs in a Windows library and is not in this build.
- Factory Reset and Manual Update are disabled. Factory reset is never sent.
- The record, snapshot, and folder buttons are in the toolbar. A virtual camera is not part of this build. The line above the preview says so.

## Run it on Ubuntu

Built and run on Ubuntu 26.04.1. The panel is a window, so run the installer from a desktop session. Plug the Tiny 2 Lite in first. Ubuntu binds `uvcvideo` on its own.

```sh
git clone https://github.com/tehlulzywolf/obsbot-center-linux.git
cd obsbot-center-linux
./install.sh
```

`install.sh` asks for your password when it needs sudo. It then:

- installs the compiler and the window libraries
- installs Rust stable when the compiler is missing or older than 1.81
- adds your user to the `video` group when you are not already in it
- builds the release panel
- adds an OBSBOT Center entry to the application menu
- starts the window

Run `install.sh` again after you pull new code. It builds again and refreshes the menu entry. The command it installs is `~/.local/bin/obsbot-center`.

The panel scans `/dev/video0` through `/dev/video15` for the node whose name contains `OBSBOT`. If the preview stays on "waiting for the camera" after the installer added you to `video`, log out and back in. Leave the preview at 30 fps. A 60 fps preview on a usbip link has dropped the camera while a vendor write was in flight.

### Tests

The tests do not open the camera.

```sh
cd obsbot-core
cargo test
cargo test --example panel --features panel
```

The first command is the codec and device tests. The second runs the panel's unit tests.

## From Windows, through WSL

The same Ubuntu userland runs under WSL2. Attach the camera from Windows PowerShell with [usbipd-win](https://github.com/dorssel/usbipd-win), using the bus id `usbipd list` prints for `3564:fef9`:

```powershell
usbipd list
usbipd attach --wsl --busid <BUSID>
```

`usbipd detach --busid <BUSID>` gives the camera back to Windows. Inside Ubuntu, run `./install.sh`. WSLg provides the display.

`obsbot-core/run_panel.sh` is only for that WSLg case. It restarts the already-built debug binary if WSLg drops the connection, and it points the process at WSLg's runtime directory. Do not use it on a normal Ubuntu desktop.

## Repository

| Path | What it is |
|---|---|
| `install.sh` | Installs dependencies, builds the panel, and starts it |
| `obsbot-core/` | The Rust crate, the panel, and the tests |
| `docs/images/` | The screenshots above |

## License

MIT. See [LICENSE](LICENSE).
