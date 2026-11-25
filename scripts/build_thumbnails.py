"""Genera miniaturas WebP de 128 px para el instalador a partir de la caché de la app.

Origen: outputs/LocalSkins/data/thumbnails/*.png (no se modifica).
Destino: work/package-thumbnails/*.webp (caché incremental: solo convierte las nuevas).
Se ejecuta desde la raíz del proyecto.
"""
import concurrent.futures
import json
from pathlib import Path

from PIL import Image

SOURCE = Path('outputs/LocalSkins/data/thumbnails')
TARGET = Path('work/package-thumbnails')
MAX_SIZE = 128


def convert(path):
    out = TARGET / (path.stem + '.webp')
    if not out.exists():
        with Image.open(path) as im:
            im = im.convert('RGBA')
            im.thumbnail((MAX_SIZE, MAX_SIZE), Image.Resampling.LANCZOS)
            im.save(out, 'WEBP', quality=82, method=6)
    with Image.open(out) as im:
        if im.format != 'WEBP' or max(im.size) > MAX_SIZE:
            raise ValueError('Miniatura inválida: ' + str(out))
    return path.stat().st_size, out.stat().st_size


def main():
    if not SOURCE.is_dir():
        raise SystemExit(f'No existe {SOURCE}: abre la app al menos una vez para crear la caché de miniaturas.')
    TARGET.mkdir(parents=True, exist_ok=True)
    with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool:
        results = list(pool.map(convert, sorted(SOURCE.glob('*.png'))))
    report = {'count': len(results), 'source_bytes': sum(a for a, _ in results),
              'bundled_bytes': sum(b for _, b in results)}
    print(json.dumps(report))


if __name__ == '__main__':
    main()
