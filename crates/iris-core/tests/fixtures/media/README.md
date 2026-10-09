# Synthetic media fixtures

These files are generated color quadrants, contain no user photos, and are
dedicated to the public domain under CC0-1.0.

`quadrants.png` is a 64 x 40 RGB raster: top left (230,20,30), top right
(20,220,30), bottom left (30,40,230), bottom right (220,210,20).
`quadrants.jpg` (quality 95) and lossless `quadrants.webp` were encoded from that
PNG using Pillow for mixed-format HTTP tests.

HEIC files were encoded locally using pillow-heif 1.3.0 (test preparation only;
not a product dependency), quality 100. `quadrants-rotated.heic` was saved with
EXIF Orientation 6, which the encoder represents with the corresponding HEIF
item transformation. Its decoded display dimensions are 40 x 64. Product tests
decode these through the bundled libheif/libde265 DLLs, not Python or ImageMagick.

Regeneration with Pillow and pillow-heif:

```python
from PIL import Image
import pillow_heif
im = Image.open("quadrants.png")
heif = pillow_heif.from_pillow(im)
heif.save("quadrants.heic", quality=100)
exif = Image.Exif()
exif[274] = 6
heif.save("quadrants-rotated.heic", quality=100, exif=exif.tobytes())
```
