import struct, sys

class CFB:
    def __init__(self, path):
        self.f = open(path, 'rb')
        h = self.f.read(512)
        assert h[:8] == bytes.fromhex('d0cf11e0a1b11ae1')
        self.major = struct.unpack_from('<H', h, 0x1A)[0]
        self.ss = 1 << struct.unpack_from('<H', h, 0x1E)[0]
        self.mss = 1 << struct.unpack_from('<H', h, 0x20)[0]
        n_fat, first_dir, _, self.mini_cutoff, first_mfat, n_mfat, first_difat, n_difat = \
            struct.unpack_from('<IIIIIIII', h, 0x2C)
        difat = list(struct.unpack_from('<109I', h, 0x4C))
        s = first_difat
        for _ in range(n_difat):
            d = self.sector(s)
            vals = struct.unpack('<%dI' % (self.ss // 4), d)
            difat += vals[:-1]; s = vals[-1]
        difat = [x for x in difat if x < 0xFFFFFFFA][:n_fat]
        self.fat = []
        for sec in difat:
            self.fat += struct.unpack('<%dI' % (self.ss // 4), self.sector(sec))
        self.dirs = self.parse_dirs(self.chain_bytes(first_dir))
        root = self.dirs[0]
        self.ministream = self.chain_bytes(root['start'])[:root['size']] if root['size'] else b''
        self.mfat = []
        if n_mfat:
            d = self.chain_bytes(first_mfat)
            self.mfat = list(struct.unpack('<%dI' % (len(d) // 4), d))

    def sector(self, n):
        self.f.seek((n + 1) * self.ss)
        return self.f.read(self.ss)

    def chain(self, start, fat):
        out = []; s = start; seen = set()
        while s < 0xFFFFFFFA and s not in seen:
            seen.add(s); out.append(s); s = fat[s]
        return out

    def chain_bytes(self, start):
        return b''.join(self.sector(s) for s in self.chain(start, self.fat))

    def parse_dirs(self, data):
        dirs = []
        for i in range(len(data) // 128):
            e = data[i * 128:(i + 1) * 128]
            nl = struct.unpack_from('<H', e, 64)[0]
            name = e[:max(nl - 2, 0)].decode('utf-16le', 'replace')
            typ = e[66]
            left, right, child = struct.unpack_from('<III', e, 68)
            start = struct.unpack_from('<I', e, 116)[0]
            size = struct.unpack_from('<Q', e, 120)[0]
            if self.major == 3: size &= 0xFFFFFFFF
            dirs.append(dict(i=i, name=name, type=typ, left=left, right=right, child=child, start=start, size=size))
        return dirs

    def children(self, idx):
        out = []
        def walk(n):
            if n >= len(self.dirs) or n == 0xFFFFFFFF: return
            d = self.dirs[n]; walk(d['left']); out.append(d); walk(d['right'])
        walk(self.dirs[idx]['child'])
        return out

    def tree(self, idx=0, prefix=''):
        res = []
        for c in sorted(self.children(idx), key=lambda d: d['name']):
            path = prefix + '/' + c['name']
            res.append((path, c))
            if c['type'] == 1: res += self.tree(c['i'], path)
        return res

    def read(self, d):
        if d['size'] < self.mini_cutoff:
            ch = self.chain(d['start'], self.mfat)
            return b''.join(self.ministream[s * self.mss:(s + 1) * self.mss] for s in ch)[:d['size']]
        return self.chain_bytes(d['start'])[:d['size']]

if __name__ == '__main__':
    c = CFB(sys.argv[1])
    print('CFB v%d, sector %d, mini %d, cutoff %d, dirs %d' % (c.major, c.ss, c.mss, c.mini_cutoff, len(c.dirs)))
    for path, d in c.tree():
        print('%-4s %12d  %s' % ('DIR' if d['type'] == 1 else 'STR', d['size'], path))
