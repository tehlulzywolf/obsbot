import re, struct, uuid
for name in ["USBPcap.sys", "USBPcapCMD.exe"]:
    b = open("/mnt/c/Program Files/USBPcap/" + name, "rb").read()
    found = set()
    # mixed-endi GUID: first 3 fields LE, last 8 bytes BE
    for off in range(0, len(b) - 16):
        raw = b[off:off+16]
        u = uuid.UUID(bytes_le=raw[:8] + raw[8:10][::-1] + raw[10:16])
        s = str(u)
        # heuristics: known windows GUIDs are noise; collect all and print with offset for eyeball
    # better: search ASCII/UTF16 strings for guid-format text
    for m in re.finditer(rb"\{?([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})\}?", b):
        found.add(("ascii", m.group(1).lower()))
    for m in re.finditer(rb"(?:[\x20-\x7e]\x00){8,}", b):
        s = m.group(0).decode("utf-16-le", "ignore")
        for g in re.findall(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}", s):
            found.add(("utf16", g.lower()))
    # also: interesting device-path-like strings in utf16
    u = b.decode("utf-16-le", "ignore")
    paths = sorted(set(re.findall(r"[\\\\#][A-Za-z0-9_#$\\\\.-]{6,60}", u)))
    interesting = [p for p in paths if re.search(r"(?i)pcap|usb|urb|pipe|control", p)][:25]
    print("==", name)
    print("GUID-strings:", sorted(found)[:15])
    print("paths:", interesting)
