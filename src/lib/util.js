/** Small formatting helpers shared by views. Timestamps from the backend are unix SECONDS. */
export const fmtMs = (ms) => {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
};
export const fmtClock = (d = new Date()) =>
  d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit', hour12: false });
export const fmtTs = (ts) =>
  new Date(ts * 1000).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
export const startOfDay = (d = new Date()) => {
  const x = new Date(d);
  x.setHours(0, 0, 0, 0);
  return x;
};
export const addDays = (d, n) => {
  const x = new Date(d);
  x.setDate(x.getDate() + n);
  return x;
};
export const secs = (d) => Math.floor(d.getTime() / 1000);
export function ago(ts) {
  const s = Math.floor(Date.now() / 1000 - ts);
  if (s < 60) return 'just now';
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  return `${Math.floor(s / 86400)} d ago`;
}
