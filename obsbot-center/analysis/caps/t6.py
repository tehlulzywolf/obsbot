import json
tx = json.load(open('/mnt/c/Users/WOLF/RE-workspace/obsbot-center/analysis/caps/tx9.json'))
for t in tx[:6]:
    print('fr', t['fr'], 'wL', t['wL'], 'bmt', hex(t['bmt']), 'outlen', len(t['out']) // 2, 'inlen', len(t['in']) // 2)
    print('  out:', t['out'][:40])
    print('  in :', t['in'][:40])
