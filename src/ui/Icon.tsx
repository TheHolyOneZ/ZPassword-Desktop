const paths = {
  key: "M7.5 12.5a3.5 3.5 0 1 1 0-7 3.5 3.5 0 0 1 0 7Zm3.5-3.5h7m-2.5 0v3m-2.5-3v2",
  database: "M4 5c0-1.4 2.7-2 6-2s6 .6 6 2-2.7 2-6 2-6-.6-6-2Zm0 0v10c0 1.4 2.7 2 6 2s6-.6 6-2V5M4 10c0 1.4 2.7 2 6 2s6-.6 6-2",
  server: "M3.5 3.5h13v5h-13zm0 8h13v5h-13zM6 6h1m-1 8h1",
  code: "m7 6-4 4 4 4m6-8 4 4-4 4M11 4 9 16",
  wifi: "M2.5 8a11 11 0 0 1 15 0M5 10.8a7.3 7.3 0 0 1 10 0m-7.4 2.7a3.4 3.4 0 0 1 4.8 0M10 16.2h.01",
  card: "M2.5 5h15v10h-15zm0 3h15M5.5 12.5h3",
  id: "M3 4.5h14v11H3zm3 3.5a1.8 1.8 0 1 0 3.6 0A1.8 1.8 0 0 0 6 8Zm-1 5c.6-1.4 1.6-2 2.8-2s2.2.6 2.8 2M12 8h3m-3 3h3",
  note: "M5 2.5h7l3.5 3.5v11.5H5zM12 2.5V6h3.5M7.5 10h5m-5 3h5",
  folder: "M2.5 5.5v10h15v-8.5H9.5L8 5.5z",
  label: "M3 3h7l7 7-7 7-7-7zm4 4h.01",
  star: "m10 2.8 2.2 4.6 5 .7-3.6 3.5.9 5-4.5-2.4-4.5 2.4.9-5L2.8 8.1l5-.7z",
  copy: "M7 7h9.5v9.5H7zM13 7V3.5H3.5V13H7",
  eye: "M1.8 10S4.5 4.5 10 4.5 18.2 10 18.2 10 15.5 15.5 10 15.5 1.8 10 1.8 10Zm8.2 2.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5Z",
  eyeOff: "M3 3l14 14M8.2 5a8 8 0 0 1 1.8-.5c5.5 0 8.2 5.5 8.2 5.5a14 14 0 0 1-2 2.8M12 12.7a2.5 2.5 0 0 1-3.6-3.4M5.3 6.4A13 13 0 0 0 1.8 10s2.7 5.5 8.2 5.5c1.3 0 2.5-.3 3.5-.8",
  plus: "M10 3.5v13M3.5 10h13",
  search: "M8.5 14a5.5 5.5 0 1 0 0-11 5.5 5.5 0 0 0 0 11Zm4-1.5 4.5 4.5",
  lock: "M4.5 9h11v8h-11zm2.5 0V6.5a3 3 0 0 1 6 0V9m-3 3v2",
  unlock: "M4.5 9h11v8h-11zm2.5 0V6.5a3 3 0 0 1 5.8-1M10 12v2",
  settings: "M10 12.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5ZM8.6 2.5h2.8l.5 2.2 1.6.9 2.1-.8 1.4 2.4-1.7 1.5v1.8l1.7 1.5-1.4 2.4-2.1-.8-1.6.9-.5 2.2H8.6l-.5-2.2-1.6-.9-2.1.8L3 13.8l1.7-1.5v-1.8L3 9 4.4 6.6l2.1.8 1.6-.9z",
  trash: "M3.5 5.5h13M8 5.5V3h4v2.5m-6.5 0 .8 11.5h7.4l.8-11.5M8.5 8.5v5.5m3-5.5v5.5",
  dice: "M3.5 3.5h13v13h-13zM7 7h.01M13 7h.01M10 10h.01M7 13h.01M13 13h.01",
  gauge: "M3 14.5a7 7 0 1 1 14 0M10 14.5l3-5M3 14.5h2m10 0h2M10 7.5V6",
  close: "m5 5 10 10M15 5 5 15",
  minimize: "M5 10h10",
  maximize: "M5 5h10v10H5z",
  chevronRight: "m8 5 5 5-5 5",
  chevronDown: "m5 8 5 5 5-5",
  check: "m4 10.5 4 4 8-9",
  alert: "M10 3 2.5 16.5h15zM10 8v4m0 2.5h.01",
  clock: "M10 17a7 7 0 1 0 0-14 7 7 0 0 0 0 14Zm0-10.5V10l2.5 2",
  history: "M3.5 10a6.5 6.5 0 1 0 2-4.7M3.5 3v3h3M10 6.5V10l2.5 2",
  download: "M10 3v10m-4-4 4 4 4-4M3.5 16.5h13",
  upload: "M10 13V3M6 7l4-4 4 4M3.5 16.5h13",
  more: "M5 10h.01M10 10h.01M15 10h.01",
  edit: "M3.5 16.5h3l9-9-3-3-9 9zm7-10 3 3",
  external: "M11 3.5h5.5V9m0-5.5L9 11m5 .5v5H3.5V6h5",
  link: "M8.5 11.5a3 3 0 0 0 4.2 0l3-3a3 3 0 0 0-4.2-4.2l-1 1m1 3.2a3 3 0 0 0-4.2 0l-3 3a3 3 0 0 0 4.2 4.2l1-1",
  shield: "M10 2.5 3.5 5v5c0 4 2.8 6.5 6.5 7.5 3.7-1 6.5-3.5 6.5-7.5V5z",
  fingerprint: "M5.5 15a9 9 0 0 1-1-4.5 5.5 5.5 0 0 1 11 0c0 1.3-.2 2.6-.6 3.8M8 16.5a12 12 0 0 1-1-5.5 3 3 0 0 1 6 0c0 1.8-.3 3.6-1 5.3M10 11c0 2-.4 4-1.2 5.8",
  usb: "M7.5 2.5h5v6h-5zm-1 6h7V15a3.5 3.5 0 0 1-7 0zM9 4.5h.01M11 4.5h.01",
  file: "M5 2.5h7l3.5 3.5v11.5H5zM12 2.5V6h3.5",
  grip: "M7.5 5h.01M12.5 5h.01M7.5 10h.01M12.5 10h.01M7.5 15h.01M12.5 15h.01",
  arrowUp: "M10 16V4m-5 5 5-5 5 5",
  arrowDown: "M10 4v12m-5-5 5 5 5-5",
  restore: "M3.5 10a6.5 6.5 0 1 0 2-4.7M3.5 3v3h3",
  vault: "M2.5 3.5h15v13h-15zM10 13a3 3 0 1 0 0-6 3 3 0 0 0 0 6Zm0-3h.01M5 16.5v1m10-1v1M2.5 7h-1m1 6h-1",
  phone: "M6 2.5h8v15H6zm3 12.5h2",
  calendar: "M3.5 4.5h13v12h-13zm0 4h13M7 2.5v4m6-4v4",
  hash: "M4 7.5h13M3 12.5h13M8 3.5 6.5 16.5m7-13L12 16.5",
  text: "M4 5.5V4h12v1.5M10 4v12.5m-2.5 0h5",
  mail: "M2.5 4.5h15v11h-15zm0 0L10 11l7.5-6.5",
  user: "M10 10a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Zm-6.5 7.5c.8-3 3.3-4.5 6.5-4.5s5.7 1.5 6.5 4.5",
  globe: "M10 17.5a7.5 7.5 0 1 0 0-15 7.5 7.5 0 0 0 0 15ZM2.5 10h15M10 2.5c2 2 3 4.5 3 7.5s-1 5.5-3 7.5c-2-2-3-4.5-3-7.5s1-5.5 3-7.5Z",
  paragraph: "M3.5 5h13m-13 4h13m-13 4h13m-13 4h8",
} as const;

export type IconName = keyof typeof paths;

export function Icon({ name, size = 16, className, title }: { name: IconName; size?: number; className?: string; title?: string }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 20 20"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="square"
      strokeLinejoin="miter"
      className={className}
      aria-hidden={title ? undefined : true}
      role={title ? "img" : undefined}
    >
      {title && <title>{title}</title>}
      <path d={paths[name]} />
    </svg>
  );
}

export const templateIcon = (icon: string): IconName => (icon in paths ? (icon as IconName) : "file");

export const fieldIcon: Record<string, IconName> = {
  text: "text",
  username: "user",
  email: "mail",
  password: "key",
  url: "globe",
  totp: "clock",
  number: "hash",
  date: "calendar",
  phone: "phone",
  multiline: "paragraph",
  hidden: "eyeOff",
};
