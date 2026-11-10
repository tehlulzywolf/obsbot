import struct
data = open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/cap1.pcap', 'rb').read(220000)
print('global:', data[0:24].hex(' '))
pos = 24
i = 0
while i < 40:
    t1, t2, cap, orig = struct.unpack_from('<IIII', data, pos)
    d = data[pos + 16:pos + 16 + cap]
    print('rec', i, 'hdr pos=', pos, 'cap=', cap, 'orig=', orig, 'first16=', d[:16].hex(' '))
    pos += 16 + cap
    i += 1
