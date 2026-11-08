/** The app mark: a clipboard on a blue tile. Same artwork as src-tauri/icons/app-icon.svg. */
export function Logo({ size = 48 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden>
      <defs>
        <linearGradient id="logo-bg" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#4f8cff" />
          <stop offset="1" stopColor="#2c5fd9" />
        </linearGradient>
      </defs>
      <rect x="4" y="4" width="56" height="56" rx="14" fill="url(#logo-bg)" />
      <rect x="18" y="16" width="28" height="34" rx="5" fill="#fff" />
      <rect x="25" y="12" width="14" height="8" rx="3" fill="#dbe6ff" stroke="#2c5fd9" strokeWidth="2" />
      <path d="M24 29h16M24 35h16M24 41h10" stroke="#2c5fd9" strokeWidth="3" strokeLinecap="round" />
    </svg>
  );
}
