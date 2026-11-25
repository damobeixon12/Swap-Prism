"""Discard disposable generated payloads only after a successful commit."""
from pathlib import Path
import shutil

def discard(root,jobs):
    base=(Path(root)/'paquetes').resolve();removed=0
    for item in jobs:
        job=Path(item).resolve()
        if not job.is_relative_to(base) or job==base:continue
        for name in ('generado','validacion','AssetSwapper_Decrypted'):
            folder=job/name
            if not folder.is_dir() or folder.is_symlink() or folder.is_junction():continue
            if folder.resolve().parent!=job:continue
            try:shutil.rmtree(folder);removed+=1
            except OSError:pass # Successful installation must stay successful.
    return removed
