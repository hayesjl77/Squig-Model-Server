import struct, zlib, os

os.chdir(os.path.dirname(os.path.abspath(__file__)))

def make_rgba_png(w, h, bg=(26, 26, 46, 255)):
    """Create a minimal valid RGBA PNG with color type 6."""
    raw = b""
    for y in range(h):
        raw += b"\x00"  # filter: none
        for x in range(w):
            raw += bytes(bg)

    def chunk(ctype, data):
        c = ctype + data
        return struct.pack(">I", len(data)) + c + struct.pack(">I", zlib.crc32(c) & 0xFFFFFFFF)

    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)  # bit depth 8, color type 6 = RGBA
    compressed = zlib.compress(raw)

    return sig + chunk(b"IHDR", ihdr) + chunk(b"IDAT", compressed) + chunk(b"IEND", b"")


for size, name in [(32, "32x32.png"), (128, "128x128.png"), (256, "128x128@2x.png")]:
    data = make_rgba_png(size, size)
    with open(name, "wb") as f:
        f.write(data)
    print(f"{name}: {len(data)} bytes, color type 6 (RGBA)")

# ICO from the 128x128
from PIL import Image
img = Image.open("128x128.png")
img.save("icon.ico", format="ICO", sizes=[(128, 128)])
print("icon.ico: done")
