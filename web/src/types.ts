export type Status = {
  version?: string;
  phase: string;
  enabled: boolean;
  hdmi: string;
  hdmi_connected?: boolean;
  usb_state?: string | null;
  bitrate_mbps: number;
  input_width?: number;
  input_height?: number;
  input_fps?: number;
  input_nominal_fps?: number;
  output_fps?: number;
  hdmi_width?: number;
  hdmi_height?: number;
  hdmi_hz?: number;
  hdmi_modes?: string[];
  temperature_c?: number;
  uptime_seconds: number;
  video_bytes: number;
  discarded_bytes?: number;
  decoder_starts?: number;
  message?: string;
  display_error?: string;
};
export type Settings = {
  hdmi_mode: string;
  fallback_image: string | null;
  preview_enabled: boolean;
};
export type ImageEntry = { id: string; url: string };
export type Sample = {
  t: number;
  bitrate_mbps: number | null;
  input_fps: number | null;
  output_fps: number | null;
  temperature_c: number | null;
  phase: string;
  hdmi: string;
};
export type History = { now: number; samples: Sample[] };
export type Wifi = {
  available: boolean;
  ssid?: string;
  applying?: boolean;
  error?: string;
};
