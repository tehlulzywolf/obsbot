# OBSBOT Center — UI reverse-engineering map (2026-11-07)

## Process topology (proven from import tables)
- **OBSBOT_Center.exe** (14.7 MB, Qt6 *Widgets* app, no QML): streaming suite UI.
  Imports **rm.dll (485 fns)** = renamed libobs (`rm_*`≈`obs_*`, `gs_*` graphics,
  scenes/sources/outputs/encoders/service, beauty hooks, ai_audio hooks) + Qt6Widgets(916)/Qt6Core(377)/Qt6Gui(251)/Qt6Network/libcurl.
  Qt6Multimedia used only for QMediaDevices/QSoundEffect (audio device lists + clicks), NOT camera video.
- **OBSBOT_Main.exe** (92 MB, Qt + OpenGLWidgets + WebSockets + SerialPort + Bluetooth + HID):
  the **device app**; imports **libdev.dll (452 named fns)** — owns OBSBOT camera.
  Video preview comes from the UVC stream rendered via Qt6OpenGLWidgets; rlottie for animations.
- libdev.dll = camera protocol core (already ported to obsbot-core).
- Full name-saved list of the 452 device functions: `main_libdev_imports.txt` (this dir).

## UI feature surface (from data/center/locale/en-US.ini, 466 keys)
Beauty(169) VideoOutput(57) Common(41) VideoPreview(34) AudioMix(30) Record(13) Loop(13)
SceneBgText(12) Material(11) Output(8) Audio(7) RecordMore(7) MainPanel(6) SystemTray(6)
VideoBeauty(6) Device(6) Ctrl(5) PIP(3) RemoteRtc(3) VirtualVoice(1) MembershipPurchase(1)

## Asset pack (reusable as-is in any rebuild)
- data/center/themes/Dark.qss (26.5 KB Qt stylesheet)
- data/center/images/*.png (icons), font/ (8 TTFs), locale/ (14 langs)

## Main.exe device feature buckets (452 libdev fns; see main_libdev_imports.txt)
- gimbal(26): aiSetGimbalSpeedCtrlR(f,f,f) ✓wire 42c4|0804; aiSetGimbalMotorAngleR(M,M,M);
  aiGetGimbalStateR ✓0818; preset list/name/add/del ✓043b; aiSetGimbalBootPosR; aiGetGimbalParaR(type enum, float/bool)
- ai(160): tracking modes, gesture track params, zone presets (aiAddZonePresetR), target select/del,
  aiGetAiStatusR(AiStatus*, callback, GetMethod), hand-track init pos, control paras
- zoom(7): get/set absolute (float), relative enum, with-speed; cameraSetZoomStopR
- exposure/focus (in device bucket): AE/AFC, absolute exposure/focus, iso thresholds
- track(8): cameraSetVirtualTrackEnabledR/Info/Gesture, AFC track mode
- audio(20) mic/AGC/volume; led(4): tally on/brightness, battery light
- preset(3): cameraGetCameraPreset/Set/Tri (PresetActions = richer than gimbal-only presets)
- net/wifi/ble: ethernet/STA/AP config, BLE pairing; upgrade(8): DevUpgrade task class
- misc: scheduled sleep/resume, NDI-vs-RTSP select, SRT config, HDMI info, image-test mode

## UVC video path on Linux (measured on WSL 6.18.54 + uvcvideo 1.1.1)
- uvcvideo binds Tiny 2 Lite: /dev/video0 (capture), /dev/video1 (metadata), /dev/media0.
- BUT no /dev/video4XU* and no /dev/v4l-subdev* char nodes on WSL kernel build
  => kernel XU passthrough unavailable => **stream must also come from userspace nusb**,
  and uvcvideo claim conflicts with nusb. Decision: nusb full-stack, uvcvideo unbound
  (WSL: interfaces simply left unbound; Lenovo Ubuntu: udev rule / blacklist).
- Streaming spec (from descriptors, USB2 HS):
  * VS if 1 alt 0, **bulk IN EP 0x82, mps 512** (bulk not iso) + status intr EP 0x83? (see 150-340 dump; VC status EP 0x86 for AA events)
  * Format 1 MJPEG: frames 1920x1080@15-60fps, 3840x2160@30-15, 1280x720@15-60
  * Format 2 YUY2 (guid 32595559-...): 640x360@30, 640x480@30
  * Standard UVC 1.0 negotiation: SET_CUR VS_PROBE_COMMIT(sel1, if1) 34B -> SET_CUR VS_COMMIT(sel2) bCommand=1, stream on bulk 0x82,
    UVC payload headers (magic 0x0f..), frame reassembly via bScEoh/bEpP markers.
- Zoom/Pan/Tilt/AE/Focus also exist as *standard UVC* controls (bmControls 0x23e3a incl PanTilt+Zoom) —
  usable as fallback for non-AA ops, but AA commands are richer.

## Rust rebuild plan (chosen toolkit: egui/eframe — WSLg-safe, pure Rust, no C deps)
1. `obsbot-gui` crate in RE-workspace/obsbot-core (or sibling), depends on obsbot-core (controls) + stream module:
   - stream.rs: UVC probe/commit + bulk EP0x82 reader + payload deframeramer (YUY2 direct, MJPEG via zune-jpeg)
   - UI panes: device status (serial/boot/attitude), gimbal D-pad + speed + angle move,
     presets save/recall/list, AI tracking on/off + mode + gesture, zoom slider, tally LED toggle
   - reuse OBSBOT Dark.qss look & image assets where practical
2. Run under WSLg (wayland auto), test against camera attached via usbipd.
