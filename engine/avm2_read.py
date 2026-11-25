"""Bounded AVM2 instruction reader; never executes bytecode."""
from hud_abc import Reader

# Operand counts from Adobe avmplus core/opcodes.tbl (MPL 2.0); not execution.
# Full decoding is necessary to avoid treating operands as instructions.
def instructions(code):
    from pathlib import Path
    import json
    import sys
    table=json.loads((Path(getattr(sys,'_MEIPASS',Path(__file__).parent))/'avm2_operands.json').read_text())
    r=Reader(code);out=[]
    while r.pos<len(code):
        start=r.pos;op=r.byte();count=table[op];args=[]
        if 0x0c<=op<=0x1a:r.skip(3)
        elif op==0x1b:r.skip(3);n=r.u30();r.skip((n+1)*3)
        elif op==0x24:args=[r.byte()]
        elif op==0xef:args=[r.byte(),r.u30(),r.byte(),r.u30()]
        elif count>=0:args=[r.u30() for _ in range(count)]
        else:raise ValueError('Instrucción AVM2 no reconocida.')
        out.append((start,r.pos,op,args))
    return out
