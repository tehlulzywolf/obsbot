import re
p = "/mnt/c/Program Files/USBPcap/USBPcap.sys"
b = open(p, "rb").read()
u = b.decode("utf-16-le", "ignore")
print("UTF16 GUIDs:", sorted(set(re.findall(r"\{[0-9a-fA-F-]{36}\}", u)))[:20])
print("ASCII GUIDs:", sorted(set(x.decode() for x in re.findall(rb"\{[0-9a-fA-F-]{36}\}", b)))[:20])
print("D16:", sorted(set(re.findall(r"Device[\\\\][A-Za-z0-9_\\\\]{2,40}", u)))[:30])
print("DA:", sorted(set(re.findall(r"Device[\\\\][A-Za-z0-9_\\\\]{2,40}", b.decode("latin1"))))[:30])
c = open("/mnt/c/Program Files/USBPcap/USBPcapCMD.exe", "rb").read().decode("utf-16-le", "ignore")
print("CMD-D16:", sorted(set(re.findall(r"Device[\\\\][A-Za-z0-9_\\\\]{2,40}", c)))[:30])
print("CMD-usbpcap-mentions:", sorted(set(re.findall(r"[\\\\.A-Za-z0-9]{0,10}usbpcap[A-Za-z0-9\\\\.]{0,30}", c, re.I)))[:30])
