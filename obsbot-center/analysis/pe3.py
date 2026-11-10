
import pefile
p = r"C:\Program Files\OBSBOT Center\bin\OBSBOT_Center.exe"
pe = pefile.PE(p)
print("entry:", hex(pe.OPTIONAL_HEADER.AddressOfEntryPoint))
print("sections:")
for s in pe.sections:
    print("  ", s.Name.rstrip(b"\0").decode(), hex(s.VirtualAddress), s.Misc_VirtualSize, s.SizeOfRawData)
print("\n== Static imports ==")
pe.parse_data_directories(directories=[pefile.DIRECTORY_ENTRY['IMAGE_DIRECTORY_ENTRY_IMPORT']])
total=0
for d in pe.DIRECTORY_ENTRY_IMPORT:
    print(f"  {d.dll.decode()}: {len(d.imports)}")
    total+=len(d.imports)
print("  TOTAL:", len(pe.DIRECTORY_ENTRY_IMPORT), "dlls,", total, "functions")
try:
    pe.parse_data_directories(directories=[pefile.DIRECTORY_ENTRY['IMAGE_DIRECTORY_ENTRY_DELAY_IMPORT']])
    if hasattr(pe,'DIRECTORY_ENTRY_DELAY_IMPORT'):
        print("\n== Delay imports ==")
        for d in pe.DIRECTORY_ENTRY_DELAY_IMPORT:
            print(f"  {d.dll.decode()}: {len(d.imports)}")
except Exception as e:
    print("delay:", e)
# guard flags
try:
    print("\nGuardCFFeature setting:", getattr(pe.OPTIONAL_HEADER,'IMAGE_DIRECTORY_ENTRY_LOAD_CONFIG',None))
except: pass
