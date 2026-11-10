# OBSBOT Tiny 2 Lite port — reverse roadmap (2026-11-09)

The queue below is the list as it stood before the eye tests. Outcomes that
changed it are in `HANDOFF.md` under Field status. Do not treat a queue item
as still open when that section already records a result.

## Already in the Rust panel — do not rebuild these
- Preview: MJPEG 1280x720 at 30 fps over V4L2.
- Gimbal: drag pad + speed, 1-degree steps on pan_absolute/tilt_absolute; click pad center to recenter. Vendor gimbalSpeedCtrlR (42 C4) does NOT move this gimbal while UVC streaming.
- Zoom: zoom_absolute 0-100. The 1.00x-4.00x label is a guess, not a device readout.
- Tracking: Human follows after work mode (2, 1), enable, and type 1. Group is still the auto-group command and was not retested after that work-mode write. Framing labels 1–4 are still numbers.
- Presets: the Image tab shows them. Goto stays on `04 3B`. Save, delete, and rename are on screen and were not used.
- Image controls when camera reports the control: AF, AE, exposure comp, anti-flicker, AWB, temperature, contrast, saturation, sharpness, hue. Face mode not sent.
- Status light + brightness; sleep/wake (DevStatus 3 and 1); auto-sleep timer (currently forced off at startup).
- Snapshot JPEG, open-save-folder, JPEG-sequence record button. Toolbar rotate and mirror are preview-only. Yaw reverse flips the pan stick in the panel and also sends `65 30`.

## Reverse queue (ordered) — deliverable per export: function address, wire pair, argument bytes, enum names+values, serial-path vs no-serial path (serial word 0 like sleep/wake)
1. Face focus / face exposure: cameraSetFaceFocusR, cameraSetAutoFocusModeR (DevAutoFocusType), cameraSetFaceAER, cameraSetExposureModeR. Face vs Global enum value; does Face replace UVC auto switch or sit on top?
2. HDR: cameraSetWdrR — confirm HDR toggle, 0/1 meaning.
3. Device mirror/rotation: cameraSetMirrorFlipR, cameraGetMirrorFlipR; 90 CCW/CW/portrait-9:16 candidates cameraSetFieldView, cameraSetMediaModeU. Preview transform is not a substitute.
4. Resolution/frame rate: the panel stays on 720p30. 4K and 1080p exist in the UVC descriptor. `cameraSetUvcModeR` and `cameraSetRecordResolutionR` domains are still unnamed. Do not assume a 4K30 default.
5. Hand tracking: done for on/off. Byte 1 follows either hand, with work mode (3, 0) and gimbal follow. Byte 0 is off. Left and Right are not a side argument. State GET `4A 30` is still unverified.
6. Framing + target names: aiSetTrackingModeR takes AiVerticalTrackType (0=full body); tracking_type ids 2,3,4,5,0xA,0xB,0xC unnamed. Decompile both enums. Do NOT rewrite tracking_type byte layout. Any id outside table incl 0x1E is auto-group.
7. Click-person-to-track: aiSetSelectTargetByPos, aiSetSelectTargetByBox. Coords are in VIDEO FRAME space, not window pixels.
8. Preset save/rename/delete: preset_add exists in cmd.rs, not wired. aiDelGimbalPresetR, aiSetGimbalPresetNameWithIdR, Center add-vs-update dropdown path; aiSetGimbalBootPosR, aiTrgGimbalBootPosR if preset = power-on pos.
9. Real zoom readout: cameraGetZoomAbsoluteR, cameraGetZoomParamR, cameraSetZoomAbsoluteR. Replace 1+val/100*3 label with camera ratio; keep sending zoom_absolute unless vendor float is what Center writes while streaming.
10. Audio (order): cameraSetAudioNoiseReduceR (weak/mid/strong), cameraSetAudioAGCR or cameraTXSetAudioGainR, cameraSetAudioMicDisabledU, cameraSetAudioDistanceU, cameraSetMicrophoneDuringSleepU, cameraSetAudioVolumeR.
11. Sleep background: cameraSetBgEnableU, cameraSetBgModeU (MediaBgMode), cameraSetBgColorU, custom-image upload path; cameraSetDisableSleepWithoutStreamU; scheduled sleep set/get pair.
12. Gimbal reverse: `65 30` is sent. The acknowledgement does not invert absolute pan. The panel negates pan only. Tilt is not reversed.
13. Beauty: not USB cmds under those names. Decide on-camera (cameraSetImageStyleR / cameraSetImageStyleModeR / cameraSetGammaUserModeR) vs host filter in rm.dll (Center's OBS layer). If host-side -> video-filter port, not XU.
14. Track speed/offsets/gestures: aiSetTrackSpeedTypeR, aiSetHorizontalOffset, aiSetVerticalOffset, cameraSetGestureControlU. Zone track (aiSetLimitedZoneTrack*) later.
15. Status read-back: 0x89 async event decode (see notes) + aiGetAiStatusR so UI follows device state. cameraGetTallyAndBatteryLightR silent on this model.

## Linux-app work (not USB): virtual camera; real video file recording; format switch without killing control conn; factory reset (cameraSetRestoreFactorySettingsR — confirm payload, double-confirm UI); firmware update (DevUpgrade) later. Feedback/account/NDI/RTSP/HDMI/SDI = Center desktop features, not required.

---
## Status ledger (assistant-maintained, updated as the sweep lands)
- **DONE + wired in panel**: 1, 2, 3, 9. Findings: `roadmap_items_1_2_3_9_findings.md`.
  Item 9 fully resolved: product enum (Tail2Lite=3, dev+0x680), Center reads
  QUERYCTRL+G_CTRL zoom_absolute and shows `1+(cur-min)/(max-min)`; panel now
  uses that formula verbatim (PID 4206).
- **RE sweep COMPLETE** (subagents 2026-11-09): all of 4–8, 10–15 + factory reset
  reversed -> reports `roadmap_re_item*.md` in this directory.
- **HANDOFF.md written** — single consolidated doc for the Rust-rebuild AI:
  protocol base, every export (address/pair/args/enums/traps), factory-reset
  payload confirmed (`01 10` + byte 0x01), async 0x89 event layout, and the
  numbered UNVERIFIED-BY-EYES list that still needs the user + live camera.
- Notable corrections from the sweep: no `cameraSetTrackingTypeR` export exists
  (0x18002FD50 = `cameraSetLedCtrlU` sel6 0x0118); "kind 0x0C/0D/10" = struct
  lengths; sel6 `0x0111`/`0x011C` are inverted-semantics fields; beauty is
  host-side (Center OBS filter), not a wire command.
- Remaining after sweep: wiring in Rust + enum values that need eyeballs
  (marked UNVERIFIED-BY-EYES in reports). Linux-app backlog unchanged.
