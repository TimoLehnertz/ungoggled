#!/usr/bin/env python3
"""HTTP/settings + real decoder/preview smoke test. No Pi or USB required.
Usage: python3 tests/smoke.py target/debug/ungoggled [capture.h264]
Requires GStreamer with avdec_h264/jpegenc when a capture is supplied.
"""
import json, os, pathlib, socket, struct, subprocess, sys, tempfile, threading, time, urllib.request, urllib.error, zlib

def png():
    def chunk(kind,data): return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
    return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',2,1,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(b'\0\xff\0\0\0\xff\0'))+chunk(b'IEND',b'')

with tempfile.TemporaryDirectory(prefix='ungoggled-smoke-') as temp:
    root=pathlib.Path(temp)
    with socket.socket() as s: s.bind(('127.0.0.1',0));port=s.getsockname()[1]
    url=f'http://127.0.0.1:{port}/api/'
    def request(path,body=None,method=None,content='application/json',guard=True):
        headers={'Content-Type':content}
        if guard:headers['X-DJI-Control']='1'
        if isinstance(body,dict):body=json.dumps(body).encode()
        with urllib.request.urlopen(urllib.request.Request(url+path,body,headers,method=method),timeout=5) as r:
            b=r.read();return json.loads(b) if r.headers.get_content_type()=='application/json' else b
    def rejected(*args,**kwargs):
        try:request(*args,**kwargs)
        except urllib.error.HTTPError as e:return e.code
        raise AssertionError('request unexpectedly accepted')
    log=open(root/'server.log','w+')
    proc=subprocess.Popen([sys.argv[1],'serve','--no-autostart','--output','test','--decoder','avdec_h264','--listen',f'127.0.0.1:{port}','--data-dir',str(root/'data'),'--runtime-dir',str(root/'run'),'--web-dir','web/dist'],stdout=log,stderr=log)
    try:
        for _ in range(100):
            try:request('status');break
            except (urllib.error.URLError,ConnectionError):time.sleep(.05)
        assert rejected('stop',{},'POST',guard=False)==403
        settings=request('settings')
        assert rejected('settings',{**settings,'hdmi_mode':'1920x1080@999'},'POST')==400
        boundary='smoke-boundary'
        data=b'--'+boundary.encode()+b'\r\nContent-Disposition: form-data; name="image"; filename="test.png"\r\nContent-Type: image/png\r\n\r\n'+png()+b'\r\n--'+boundary.encode()+b'--\r\n'
        uploaded=request('images',data,'POST','multipart/form-data; boundary='+boundary)
        ident=uploaded['id'];settings['fallback_image']=ident
        request('settings',settings,'POST')
        assert request('settings')['fallback_image']==ident
        assert request('images/'+ident).startswith(b'\x89PNG')
        assert rejected('images/'+ident,method='DELETE')==400
        assert request('preview.jpg')==b''
        if len(sys.argv)>2:
            content=pathlib.Path(sys.argv[2]).read_bytes()
            error=[]
            def feed():
                try:
                    sock=socket.socket(socket.AF_UNIX,socket.SOCK_DGRAM);sock.connect(str(root/'run/video.sock'))
                    for seq,offset in enumerate(range(0,len(content),32768)):
                        b=content[offset:offset+32768];sock.send(struct.pack('<Q',seq)+b);time.sleep(len(b)/750000)
                    sock.close()
                except Exception as e:error.append(e)
            thread=threading.Thread(target=feed);thread.start()
            seen=False
            for _ in range(100):
                time.sleep(.1)
                frame=request('preview.jpg')
                if frame.startswith(b'\xff\xd8'):
                    seen=True;assert request('status')['input_width']==1920;break
            assert seen,'decoder produced no browser preview'
            thread.join();assert not error,error
            time.sleep(2)
            assert request('preview.jpg')==b'','stale preview remained after signal loss'
            assert request('status')['output_fps']==0
            assert len(request('history')['samples'])>=3
        settings['fallback_image']=None;request('settings',settings,'POST');request('images/'+ident,method='DELETE')
        print('PASS: controls, settings, image conversion/library, history'+(', H.264 decoding, JPEG preview, signal-loss expiry' if len(sys.argv)>2 else ''))
    except Exception:
        log.flush();log.seek(0);print(log.read()[-10000:]);raise
    finally:
        proc.terminate()
        try:proc.wait(timeout=5)
        except subprocess.TimeoutExpired:proc.kill();proc.wait()
