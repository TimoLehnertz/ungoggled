import React, {useEffect, useState} from 'react';
import {createRoot} from 'react-dom/client';
import './style.css';

type Status = {phase:string;enabled:boolean;video_bytes:number;bitrate_mbps:number;uptime_seconds:number;hdmi:string;output_fps?:number;hdmi_width?:number;hdmi_height?:number;hdmi_hz?:number;input_width?:number;input_height?:number;output_frames?:number;message?:string;control_packets?:number;discarded_bytes?:number};
type Diagnostics = {controllers:string[];connectors:{name:string;status:string;monitor_detected:boolean;modes:string[];id:string}[];architecture:string;gadgetfs_mounted:boolean};
const names:Record<string,string>={stopped:'Receiver stopped',starting:'Starting receiver',waiting_usb:'Waiting for goggles',usb_connected:'Goggles connected',aoa_negotiation:'Connecting to goggles',accessory:'Connecting to goggles',waiting_video:'Waiting for camera',streaming:'Receiving live video',retrying:'Reconnecting'};

function App(){
  const [status,setStatus]=useState<Status|null>(null);
  const [diagnostics,setDiagnostics]=useState<Diagnostics|null>(null);
  const [error,setError]=useState('');
  const [busy,setBusy]=useState(false);
  useEffect(()=>{
    const controller=new AbortController();let timer:ReturnType<typeof setTimeout>;let count=0;
    async function poll(){
      try{
        const r=await fetch('/api/status',{signal:controller.signal});if(!r.ok)throw new Error(`Status request failed (${r.status})`);
        setStatus(await r.json());setError('');
        if(count++%5===0){const d=await fetch('/api/diagnostics',{signal:controller.signal});if(d.ok)setDiagnostics(await d.json());}
      }catch(e){if(!controller.signal.aborted)setError(e instanceof Error?e.message:'Connection lost');}
      if(!controller.signal.aborted)timer=setTimeout(poll,1000);
    }
    void poll();
    return()=>{controller.abort();clearTimeout(timer)};
  },[]);
  async function action(action:string){
    setBusy(true);
    try{const r=await fetch(`/api/${action}`,{method:'POST',headers:{'X-DJI-Control':'1'}});if(!r.ok)throw new Error(`Could not ${action} receiver`);}
    catch(e){setError(e instanceof Error?e.message:'Command failed')}
    finally{setBusy(false)}
  }
  const live=status?.phase==='streaming'&&!error;
  const connected=diagnostics?.connectors.filter(c=>c.monitor_detected)??[];
  return <main className="mx-auto max-w-6xl px-6 py-8 md:px-12 md:py-12">
    <header className="flex items-center justify-between border-b border-white/10 pb-7">
      <a href="/" className="flex items-center gap-3 font-semibold tracking-tight text-lg"><span className="mark">↗</span> DJI HDMI <span className="ml-2 hidden text-xs font-normal tracking-widest text-stone-500 sm:inline">GROUND STATION</span></a>
      <span className="rounded-full border border-white/10 px-3 py-1.5 font-mono text-xs text-stone-400">LOCAL · USB → HDMI</span>
    </header>
    <section className="pb-8 pt-12 md:pt-16">
      <p className="eyebrow">GOGGLES 3 <span className="text-stone-600"> / </span> VIDEO BRIDGE</p>
      <div className="mt-4 flex flex-wrap items-end justify-between gap-6">
        <div><h1 className="text-4xl font-medium tracking-tight md:text-5xl">Your view. On the big screen.</h1><p className="mt-4 max-w-xl text-stone-400">A direct connection from your goggles to your HDMI display.</p></div>
        <div className="flex items-center gap-2 text-sm text-stone-400"><i className={`dot ${error?'bg-red-400':live?'bg-lime-300':'bg-amber-300'}`}/>{error?'Station unreachable':status?'Station online':'Connecting'}</div>
      </div>
    </section>
    {error&&<div role="alert" className="mb-5 rounded-xl border border-red-400/30 bg-red-400/10 p-4 text-sm text-red-200">{error}. Check the station connection.</div>}
    <section className="grid gap-5 lg:grid-cols-[1.7fr_1fr]">
      <div className="panel overflow-hidden">
        <div className="flex justify-between border-b border-white/10 px-6 py-4"><span className="eyebrow">SIGNAL STATUS</span><span className="font-mono text-xs text-stone-500">01 / INPUT</span></div>
        <div className="signal-grid flex min-h-72 flex-col items-center justify-center px-6 py-9 text-center">
          <div className={`mb-6 flex h-16 w-16 items-center justify-center rounded-2xl border text-3xl ${live?'border-lime-300/30 bg-lime-300/10 text-lime-300':'border-white/15 bg-white/5 text-stone-500'}`}>{live?'↗':'⌁'}</div>
          <h2 aria-live="polite" className="text-2xl font-medium tracking-tight">{error?'Connection lost':status?(names[status.phase]??status.phase):'Connecting to station'}</h2>
          <p className="mt-3 max-w-sm text-sm leading-6 text-stone-400">{live?'Video is arriving from the goggles. Check the connected HDMI display for the picture.':'Connect the goggles to the Pi’s USB-C port and make sure the air unit has a live picture.'}</p>
        </div>
        <div className="grid grid-cols-3 border-t border-white/10">
          <Metric label="VIDEO BITRATE" value={error?'—':(status?.bitrate_mbps??0).toFixed(2)} unit="Mbps"/>
          <Metric label="RECEIVED" value={((status?.video_bytes??0)/1e6).toFixed(1)} unit="MB"/>
          <Metric label="STATION UPTIME" value={Math.floor((status?.uptime_seconds??0)/60).toString()} unit="min"/>
        </div>
      </div>
      <aside className="panel p-6">
        <p className="eyebrow">OUTPUT CONTROL</p><h2 className="mt-3 text-xl font-medium">HDMI display</h2>
        <div className="my-6 rounded-xl border border-white/10 bg-black/20 p-4">
          <div className="flex items-center justify-between text-sm"><span className="text-stone-400">Display connection</span><span>{diagnostics?(connected.length?'Connected':'No display'):'Checking'}</span></div>
          <div className="mt-3 flex items-center justify-between text-sm"><span className="text-stone-400">Decoder</span><span className="capitalize">{error?'Unknown':status?.hdmi??'Idle'}</span></div>
          <div className="mt-3 flex items-center justify-between text-sm"><span className="text-stone-400">HDMI signal</span><span>{status?.hdmi_width?`${status.hdmi_width} × ${status.hdmi_height} · ${status.hdmi_hz} Hz`:'—'}</span></div>
          <div className="mt-3 flex items-center justify-between text-sm"><span className="text-stone-400">Camera video</span><span>{status?.input_width?`${status.input_width} × ${status.input_height}`:'—'}</span></div>
          <div className="mt-3 flex items-center justify-between text-sm"><span className="text-stone-400">Output frame rate</span><span>{!error&&status?.hdmi==='playing'?(status.output_fps??0).toFixed(1)+' fps':'—'}</span></div>
          <p className="mt-4 border-t border-white/10 pt-3 font-mono text-xs text-stone-500">{connected[0]?.name??'Use the HDMI0 port next to USB-C'}</p>
        </div>
        <button disabled={busy||!!error||!status} onClick={()=>void action(status?.enabled?'stop':'start')} className="primary w-full">{status?.enabled?'Stop receiver':'Start receiver'} <span>↗</span></button>
        <button disabled={busy||!!error||!status} onClick={()=>void action('restart')} className="secondary mt-3 w-full">Reconnect goggles</button>
        <p className="mt-5 text-xs leading-5 text-stone-500">Reconnecting briefly interrupts the video output.</p>
      </aside>
    </section>
    <section className="mt-5 grid gap-5 md:grid-cols-3">
      {[['01','Connect the air unit','O3 or O4 Air Unit Pro. Confirm the live camera image is visible inside your Goggles 3.'],['02','Connect the goggles','Use a USB-C data cable to the Pi’s USB-C port. Power the Pi separately through GPIO or PoE.'],['03','Watch on HDMI','Connect your display to HDMI0. Start the receiver above and watch the signal status.']].map(([n,title,body])=><div key={n} className="panel p-6"><span className="font-mono text-xs text-lime-300">{n}</span><h3 className="mb-2 mt-4 text-sm font-medium">{title}</h3><p className="text-sm leading-6 text-stone-500">{body}</p></div>)}
    </section>
    <section className="panel mt-5 p-6"><h3 className="text-sm font-medium">Clean video for streaming</h3><p className="mt-2 text-sm leading-6 text-stone-400">In the goggles, open Settings → Camera → Advanced Camera Settings and turn <strong className="font-medium text-stone-200">Camera View Recording off</strong> to send video without the goggles’ overlays. Turn it on when you want those overlays included.</p></section>
    <details className="mt-8 rounded-xl border border-white/10 px-5 py-4 text-sm text-stone-400"><summary className="cursor-pointer">Connection diagnostics</summary><dl className="mt-5 grid gap-3 font-mono text-xs sm:grid-cols-2"><div>USB controller: {diagnostics?.controllers.join(', ')||'Unavailable'}</div><div>Control packets: {status?.control_packets??0}</div><div>Resync bytes: {status?.discarded_bytes??0}</div><div>Last event: {status?.message??'None'}</div></dl><a className="mt-4 inline-block text-lime-300 underline" href="/api/diagnostics" target="_blank" rel="noreferrer">Open diagnostic report ↗</a></details>
    <footer className="mt-8 flex flex-wrap justify-between gap-3 text-xs text-stone-600"><span>DJI HDMI · Experimental hardware support</span><span>Independent project · Not affiliated with DJI</span></footer>
  </main>
}
function Metric({label,value,unit}:{label:string;value:string;unit:string}){return <div className="border-r border-white/10 px-4 py-5 last:border-0 md:px-6"><p className="text-[9px] tracking-widest text-stone-500 md:text-[10px]">{label}</p><p className="mt-2 font-mono text-xl md:text-2xl">{value}<span className="ml-1.5 text-xs text-stone-500">{unit}</span></p></div>}
createRoot(document.getElementById('root')!).render(<React.StrictMode><App/></React.StrictMode>);
