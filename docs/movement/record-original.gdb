set pagination off
set confirm off
set breakpoint pending on
set debuginfod enabled off
set follow-fork-mode parent
set detach-on-fork on
set environment FONTCONFIG_FILE /tmp/portal-reference-fonts.conf
set environment LD_LIBRARY_PATH /home/lars/.local/share/Steam/steamapps/common/Portal/bin:/home/lars/.local/share/Steam/steamapps/common/SteamLinuxRuntime/steam-runtime/usr/lib/i386-linux-gnu:/home/lars/.local/share/Steam/steamapps/common/SteamLinuxRuntime/steam-runtime/lib/i386-linux-gnu
python
import gdb, struct, os
log=open('/tmp/portal-measured-controlled.csv','w',buffering=1)
log.write('tick,buttons,maxspeed,suit,ducked,ducking,induckjump,jumptime,duckjumptime,originx,originy,originz,velx,vely,velz,flags\n')
count=0
installed=False
previous=0
def read(addr,fmt):
 return struct.unpack('<'+fmt,gdb.selected_inferior().read_memory(addr,struct.calcsize('<'+fmt)))[0]
class EndMove(gdb.FinishBreakpoint):
 def __init__(self,p,m):
  super().__init__(gdb.newest_frame(),internal=True); self.p=p;self.m=m
 def stop(self):
  global count,previous
  p,m=self.p,self.m;count+=1
  values=[count,read(m+0x24,'I'),read(p+0xdd0,'f'),read(p+0x8f9,'B'),read(p+0x8c0,'B'),read(p+0x8c1,'B'),read(p+0x8c2,'B'),read(p+0x8cc,'f'),read(p+0x8c8,'f')]
  values += [read(m+x,'f') for x in (0x98,0x9c,0xa0,0x40,0x44,0x48)]
  values += [read(p+0x118,'I')]
  log.write(','.join(map(str,values))+'\n')
  previous=read(m+0x28,'I')
  return False
class Move(gdb.Breakpoint):
 def stop(self):
  s=int(gdb.parse_and_eval('$esp'));p=read(s+8,'I');m=read(s+12,'I')
  tick=count+1;buttons=0;forward=0.0
  if 101<=tick<=145:buttons|=2
  if 200<=tick<=289:buttons|=8;forward=450.0
  if 250<=tick<=289:buttons|=2
  if 350<=tick<=439:buttons|=16;forward=-450.0
  if 400<=tick<=439:buttons|=2
  if 500<=tick<=599:buttons|=4
  if 550<=tick<=599:buttons|=2
  if 650<=tick<=749:buttons|=4
  if 690<=tick<=719:buttons|=8;forward=450.0
  inf=gdb.selected_inferior()
  inf.write_memory(m+0x24,struct.pack('<II',buttons,previous))
  inf.write_memory(m+0x2c,struct.pack('<fff',forward,0,0))
  inf.write_memory(m+0xc,struct.pack('<fff',0,0,0))
  EndMove(p,m)
  return False
def loaded(event):
 global installed
 if installed or not event.new_objfile.filename.endswith('/server.so'):return
 pid=gdb.selected_inferior().pid
 for line in open('/proc/%s/maps'%pid):
  if '/server.so' in line and line.split()[2]=='00000000':
   base=int(line.split('-')[0],16);Move('*%#x'%(base+0x2ce200),internal=True);installed=True
   print('REFERENCE SERVER BASE',hex(base));break
gdb.events.new_objfile.connect(loaded)
end
run
