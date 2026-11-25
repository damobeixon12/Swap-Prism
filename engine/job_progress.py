"""JSON progress frames on the private worker pipe; never a network service."""
import json,sys
def report(phase,current,total,message):
    if getattr(sys,'frozen',False):
        print(json.dumps({'progress':{'phase':phase,'current':current,'total':total,'message':message}},ensure_ascii=True),file=sys.__stdout__,flush=True)
