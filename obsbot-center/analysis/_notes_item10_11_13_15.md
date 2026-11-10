# Working notes (roadmap items 10/11/13/15) — libdev.dll, doc target-2486d1f3fe8b.dll
Sender helpers: FUN_180007490 = AA GET(sender; kind 0x0C), FUN_180008180 = AA SET(sender; kinds 0x0D/0x0E/0x10/0x11), FUN_1800087f0(dev,6,buf) = sel6 60B AiMode setter, FUN_1800085e0(dev,type,buf) = host/sel get. Struct: [0]=kind, u16@+0xe=0xS_ID, u32 args @+0x1c. Copies serial(dev+0x1668) & dev_mode(dev+0x3c) into request head (+0x24/+0x28) for channel routing.

## Item 10 audio
- cameraGetAudioVolumeR 0x1800f7f40: kind 0x0C, ID 0x10073 -> pair [0x73,0x10]. Reply byte0-1 = volume u16 (short).
- cameraSetAudioVolumeR 0x1800f7ea0: kind 0x0E (SET, no pre-read), ID 0x10074 -> pair [0x74,0x10], u32@+0x1c low 16 = volume (short param). All products same (no product branch; dev_mode routing only).
- cameraGetAudioAGCR 0x1800f8a80: kind 0x0C, ID 0x10078 -> pair [0x78,0x10]; reply byte0 != 0 = AGC on.
- cameraSetAudioAGCR 0x1800f8180: 3 paths:
  * productType==4: kind 0x0D ID 0x10077 -> pair [0x77,0x10], byte@+0x1c = bool.
  * productType 0xB/0x16/0x10: pre-GET pair 0x78 then kind 0x11 ID 0x10079 -> pair [0x79,0x10]; payload +0x1c: byte0=new bool, +2/+3 = echo of prev reply bytes (reply[1],reply[3]).
  * else (Tiny/Tail incl. product 3): sel6 field id 0x0117, value byte = bool (log "cameraSetAudioAutoGainU").
- cameraSetAudioMicDisabledU 0x1800f8410: sel6 field 0x011C, value byte = !disabled (ENABLED semantics; local_58[0]=CONCAT62(!param,0x11c)).
- cameraSetAudioDistanceU 0x1800f84c0: sel6 field 0x011B, value byte = uchar distance level.
- cameraSetAudioNoiseReduceR 0x1800f8b50: dev_mode0 && product not in {4,0xB,0x16,0x10}: sel6 field 0x010A, value byte = enabled ? (char)level : 0.
  else: kind 0x11 ID 0x1007B -> pair [0x7B,0x10]; +0x1c byte0=enabled, +0x1d int=level.
- cameraGetAudioNoiseReduceR 0x1800f8d20: kind 0x0C ID 0x1007A -> pair [0x7A,0x10]; reply byte0=enabled, bytes 1-4 int=level.
- cameraSetMicrophoneDuringSleepU 0x18002ecb0: sel6 field 0x0113, value byte = (byte)int (mic-on-during-sleep flag).
- cameraSetDisableSleepWithoutStreamU 0x18002ec00: sel6 field 0x0112, value byte = bool.
- cameraTXSetAudioGainR 0x1800fd480: TX-family AA cmd: kind 0x10 SET_ACK, ID u32@+0x1c = 0x0016000B (pair [0x0B, 0x16<<4]), +0x0d = 0x15 (TX cmd family tag), +0x0f = DevTXType byte, gain int @+0x20 (u32 after ID).
- cameraTXGetAudioGainR 0x1800fd520: kind 0x0C, ID u32@+0x1c = 0x0017000B (pair [0x0B, 0x17<<4]), +0x0d=0x15, +0x0f=DevTXType; reply int@0 = gain.
NOTE: for TX cmds the u32 at +0x1c encodes sub-cmd<<16 | pair-ID (0x0B = TX accessory pair); +0x0d/+0x0f = TX device selector bytes.
DevTXType enum: {0,1} (TX-R vs TX-S? values pass-through, byte only).

### audio cont'd
- cameraSetAudioVQETypeR 0x1800f8010: kind 0x0D ID 0x10076 -> [0x76,0x10], byte = DevAudioVQEType.
- cameraGetAudioVQETypeR 0x1800f80b0: kind 0x0C ID 0x10075 -> [0x75,0x10], reply byte = enum.
- cameraGetAudioAuxInputTypeR 0x1800f8e00: kind 0x0C ID 0x1007C -> [0x7C,0x10], reply byte.
- cameraSetAudioAuxInputTypeR 0x1800f8ed0: kind 0x0D ID 0x1007D -> [0x7D,0x10], byte.
- cameraGetSelectedAudioSourceR 0x1800f9270: kind 0x0C ID 0x10082 -> [0x82,0x10], reply byte = selected source id.

- cameraSetAudioSourceR 0x1800f8f70: kind 0x0D ID 0x1007F -> [0x7F,0x10], byte = source id.
- cameraGetAudioSourceR 0x1800f9020: kind 0x0C ID 0x1007E -> [0x7E,0x10], reply = DevAudioInputSource raw 0x21 bytes copied verbatim (0x20 bytes + byte at +0x20).
- cameraSetAudioSourceMuteR 0x1800f9100: kind 0x0D ID 0x10081 -> [0x81,0x10], byte bool.
- cameraGetAudioSourceMuteR 0x1800f91a0: kind 0x0C ID 0x10080 -> [0x80,0x10], reply byte bool.
- 'beauty': string search 'eauty' in libdev.dll = NO matches (only 1 RTTI noise hit). No beauty export/string => beauty is HOST-SIDE (app filter), not a libdev wire feature. Device-side equivalents: ImageStyle/ImageStyleMode/Gamma.
- cameraSetImageStyleR 0x1800f54f0: kind 0x10 ID 0x10058 -> [0x58,0x10], u32 = DevImageStyle.
- cameraSetImageStyleModeR(enum) 0x1800f73a0: kind 0x10 ID 0x1006A -> [0x6A,0x10], u32 = DevImageStyleMode.
  cameraGetImageStyleModeR(enum&) 0x1800f7230: kind 0x0C ID 0x10069 -> [0x69,0x10] (verify reply from 0x1800f7160 twin).
  cameraGetImageStyleModeR(struct DevLiveStreamModeSetting&) 0x1800f7160: kind 0x0C ID 0x10069 -> [0x69,0x10], reply 8 bytes -> struct.
  cameraSetImageStyleModeR(struct) 0x1800f7300: (not yet decompiled; likely kind 0x10 ID 0x1006A payload 8B struct).
- cameraGetGammaUserModeR 0x1800f3c30: dev_mode0: FUN_1800085e0(dev, SELECTOR 0x10, buf) host/sel6 getter; else kind 0x0C ID 0x10048 -> [0x48,0x10]. Reply->DevGammaModeUserMode: r[0]->+0(mode flag), r[1]->+1(N points, clamp 0x16), then i<N: struct+2+i = r[3+2i]? decode loop: pDVar6 = reply+1 step 2: struct[+2+i]=r[+1+2i-? ] ... net: X[i]=reply[2+i]? and Y[i]=reply[0x18+i]? per dev_mode0 unpack from sel buf at &local_58+3.
- cameraSetGammaUserModeR 0x1800f3e20: dev_mode0: sel6 buffer: [0]=0x26, [1]=(N+1)*2, [2]=mode(param+0), [3]=N(param+1), then pairs [4+2i]=X[i](param+2+i), [5+2i]=Y[i](param+0x18+i) -> FUN_1800087f0(dev,6,buf).
  dev_mode!=0: kind = 0xc + (2+2N) (payloadLen-derived), ID u32 0x10049 -> pair [0x49, S=1]; payload @+0x1c: b0=mode,b1=N, pairs (X[i],Y[i]) i<N. Log 'CAM_SET_GAMMA_USER_MODE'.

- cameraSetImageStyleModeR(struct DevLiveStreamModeSetting) 0x1800f7300: kind 0x14, ID 0x1006A -> [0x6A,0x10], payload u64 = 8-byte struct at +0x1c.
- cameraSetAudioCtrlStateU 0x18002fbc0: sel6 field 0x0215, [2]=AudioCtrlCmdType byte, [3]=value byte.
- cameraSetAudioAutoGainU 0x18002fc70: sel6 field 0x0117 (SAME field as AGCR), value = bool.
- cameraSetAudioModeU 0x18002fe80: sel6 field 0x0222, [2],[3] = 2 bytes of AudioMode struct.
NEW sel6 field ids this pass: 0x0105 BgMode, 0x0110 BgColor, 0x0111 BgDisabled, 0x0112 DisableSleepWithoutStream, 0x0113 MicDuringSleep, 0x0117 AudioAGC/AutoGain, 0x011B AudioDistance, 0x011C MicEnabled, 0x010A NoiseReduce, 0x0026+GammaCurveLen GammaUserMode(len-encoded), 0x0215 AudioCtrlCmd, 0x0222 AudioMode.
GET-side selector for gamma dev_mode0 = FUN_1800085e0(dev, 0x10, buf) — selector 0x10 buffer, not a field-id struct.

## Item 11 background + sleep
- cameraSetBgModeU 0x18002f840: sel6 field 0x0105, value byte = MediaBgMode (pass-through).
- cameraSetBgColorU 0x18002f8f0: sel6 field 0x0110, value byte = MediaBgModeColorType (pass-through).
- cameraSetBgEnableU 0x18002f9a0: sel6 field 0x0111, value byte = !enable (field semantics = BG DISABLED: CONCAT62(!param,0x111)).
- cameraSetDisableSleepWithoutStreamU: see item 10 list (sel6 0x0112).
- cameraSetAutoSleepTimeR 0x1800fa0d0: kind 0x10 ID 0x10091 -> pair [0x91,0x10], u32@+0x1c = seconds.
- cameraGetAutoSleepTimeR 0x1800fa170: dev_mode 1/2 -> host cache dev+0xb80 (no wire); else kind 0x0C ID 0x10090 -> pair [0x90,0x10], reply u32@0.
- cameraGetScheduledSleepR 0x1800fa260: dev_mode1/2 -> 0x30-byte cache dev+0xb84; else kind 0x0C ID 0x10097 -> pair [0x97,0x10].
  Reply->DevScheduledInfo(0x30B) unpack: r[0]->+0u32, r[1]->+4u32, r[2..6)->+8u32, r[6..10)->+0xCu32, r[10..14)->+0x10u32, r[14..21)-> bytes +0x14,+0x18,+0x1C,+0x20,+0x24,+0x28,+0x2C (each u8->u32).
- cameraSetScheduledSleepR 0x1800fa3e0: dev_mode0: kind 0x21 ID 0x10098 -> pair [0x98,0x21? wire S-nibble], payload = first 0x15 bytes of DevScheduledInfo at +0x1c (packed: b0,b1,u32@struct+8? -> bytes 2-13, then struct bytes 0x14,0x18,0x1C,0x20,0x24,0x28,0x2C).
  dev_mode!=0: kind 0x3c ID 0x10095 -> pair [0x95,...], payload = full 0x30-byte struct at +0x1c.
- cameraGetScheduledResumeR 0x1800fa510: kind 0x0C ID 0x10099 -> pair [0x99,0x10]; cache dev+0xbac+ region for dev_mode1/2 (0x30B from dev+0xb8c? verify below).
  cameraSetScheduledResumeR 0x1800fa690: dev_mode0 kind 0x21 ID 0x1009A pair [0x9A..]; dev_mode!=0 kind 0x3c ID 0x10096 pair [0x96..] (same struct pattern as Sleep).
- NOTE byte0 ('kind') pattern: 0x0C = GET (hdr 12B); SET: byte0 = 13 + payloadLen (0x0D no-arg, 0x0E 1 arg-byte, 0x10 3B, 0x11 4B, 0x21 0x15B packed, 0x3c 0x30B). Mirror per-command.
- 'aiSetBgScheduleSleepR' does NOT exist in exports; the schedule-sleep API pair is cameraSet/GetScheduledSleepR above (struct DevScheduledInfo).
- Custom bg image upload: NO BgImage/WallPaper/CustomBg export. Image reaches device via file-transfer path: uploadFileToAppSd 0x180104F50, startFileUploadAsync(FileType) 0x18010650, devTransferringFileByMtp() 0x18011320, setFileUploadCallback 0x18011B30; cameraUploadSrtConfigR 0x180032DA0. (BgMode custom uses the file pushed over the MTP/file-upload channel; no dedicated sel6 field.)

- cameraTXSetAudioMuteR 0x1800fd5f0: kind 0x0D; ID u16@+0xe = 0x000B (pair 0x0B TX family), u16@+0x10 = 0x18 (sub-cmd); byte@+0xb = DevTXType, +0xa = 0x15, mute bool byte@+0x1c.
- cameraTXGetAudioMuteR 0x1800fd690: kind 0x0C; pair [0x0B, sub 0x19]; +0xb=TXType; reply byte = mute.
- TX pair map: sub 0x16 = SET gain, 0x17 = GET gain, 0x18 = SET mute, 0x19 = GET mute.
- cameraSetGammaModeR 0x1800f6f30: kind 0x10 ID 0x10066 -> [0x66,0x10], u32 = DevGammaMode.
- cameraGetGammaModeR 0x1800f6e60: kind 0x0C ID 0x10065 -> [0x65,0x10], reply byte = enum.
- Request header layout (confirmed): u16 kind @+0x00; byte @+0x0a = reply-length class; byte @+0x0b = TXType/dev selector; byte @+0x0c = 0x0B (camera-cmd family); u32 serial copy @+0x14 (dev+0x1668); u32 dev_mode copy @+0x18 (dev+0x3c); u16 pair-ID @+0x0e; u16 sub/family @+0x10 (0x10 = normal camera cmd, 0x15-0x19 = TX subs); args from +0x1c. (0xb00 u16@+0xb = bytes +0xb=0, +0xc=0x0b.)
- NOT mapped this pass (audio-adjacent, same wire pattern, IDs unrecorded): cameraGetAudioSelectR/SetAudioSelectR (0x1800FD9A0/FDA70), get/setUSBAudioParamR (0x1800FDB10/FDBB0), setTWSSoundMode (0x1800FDEA0), cameraSetAutoExposureR/getAutoExposure.

## Item 15 async events (dispatcher = FUN_180009450)
Frame header parse FUN_1800fff10; dispatcher switch on decoded (type,cmd):
- type1 cmd 0x12: FUN_180028590 "new file generate notify" -> pb (DAT_1801cb380) into Device::NewFileInfo dev+0x2288; event cb (dev+0x7e0) hdr type=0x82 tag=0x7d8.
- type1 cmd 0x2c: FUN_180011d00 (dev+0xf78=val; category dev+0x849: val 1-9->3, 0x11-0x1a->2, 0x21-0x2c->1).
- type3 cmd 0x6: FUN_180022740 time sync.
- type3 cmd 0x14: FUN_1800188c0 = AI/status push: records QPC dev+0x8b8; product 4 branch: mode=frame+0x1d -> dev+0x860 map {1->1,3->2,4->3,0xa->4,0x1e->5,0x14/15/16->0xff,def 0}; bytes frame+0x20,0x21,0x22,0x26,0x23 -> dev+0xcdc..0xcdf,0xce0; zoom float = u16@frame+0x24 * 0.01 -> dev+0xce4; +0x1d->0xcfc; +0x27->0xd00; +0x28->0xce8. Other products: byte@frame+0x20 bitfield (bit0->cdc,bit1->cdd,bit2->cde,bit3->cdf,bit4->ce1,bit5->ce2,bit6->ce0); zoom u16@frame+0x21*0.01->0xce4; +0x1e->0xd04; +0x1f->0xd08; +0x23->0xce9; +0x24->0xce8.
- type3 cmd 0x64(100): FUN_180014250; 0x74/0x75: FUN_180014310 (wakes cv dev+0x17e0); 0x67: FUN_1800178f0 "preset arrived"; 0x69: FUN_1800185c0 pb(0x14 dwords) -> dev+0xd30 pose/tracking struct.
- type6 cmds: FUN_180105e40 "ap status notify" (wifi AP), FUN_180106770.
Wire cmd/type bytes are XOR/lookup-mangled via lazy maps DAT_180218e28/e58/ed8/f08/ee8/ef8/f28 (FUN_180100d00/f70/aa0/fff10); raw 0x89 is an obfuscated wire cmd -> maps to one of the decoded cases; decoded case ids above.

## AiStatus cache (dev+0xcdc..0xd0b, aiGetAiStatusR 0x180019280) — pending full re-read of struct-copy layout
