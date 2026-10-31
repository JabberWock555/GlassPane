// Inline SVG icons: crisp at any size and no icon font to download.

const common = {
  width: 40,
  height: 40,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  "stroke-width": 1.8,
  "stroke-linecap": "round",
  "stroke-linejoin": "round",
} as const;

type Size = { size?: number };
const svg = (size?: number) => (size ? { ...common, width: size, height: size } : common);

export const Bulb = ({ size }: Size) => (
  <svg {...svg(size)}>
    <path d="M9 18h6M10 21h4" />
    <path d="M12 3a6 6 0 0 0-3.5 10.9c.6.5 1 1.2 1 2V16h5v-.1c0-.8.4-1.5 1-2A6 6 0 0 0 12 3z" />
  </svg>
);

export const Power = ({ size }: Size) => (
  <svg {...svg(size)}>
    <path d="M12 3v8" />
    <path d="M6.3 6.8a8 8 0 1 0 11.4 0" />
  </svg>
);

export const Fan = ({ size }: Size) => (
  <svg {...svg(size)}>
    <circle cx="12" cy="12" r="1.6" />
    <path d="M12 10.4c-.8-3.6.2-6.4 2.6-6.4 2.6 0 2.4 3.9-1 6.4" />
    <path d="M13.6 12.9c2.7 2.5 3.2 5.4 1.2 6.6-2.3 1.3-4-2.2-2.6-6.1" />
    <path d="M10.6 12.7c-3.5 1.1-6.4.3-6.6-2-.2-2.6 3.6-2.9 6.6.1" />
  </svg>
);

export const Moon = ({ size }: Size) => (
  <svg {...svg(size)}>
    <path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z" />
  </svg>
);

export const Thermo = ({ size }: Size) => (
  <svg {...svg(size)}>
    <path d="M10 14.8V5a2 2 0 1 1 4 0v9.8a4 4 0 1 1-4 0z" />
  </svg>
);

export const Drop = ({ size }: Size) => (
  <svg {...svg(size)}>
    <path d="M12 3s6 6.5 6 11a6 6 0 0 1-12 0c0-4.5 6-11 6-11z" />
  </svg>
);

export const Gauge = ({ size }: Size) => (
  <svg {...svg(size)}>
    <path d="M4 16a8 8 0 1 1 16 0" />
    <path d="M12 16l4-5" />
  </svg>
);

// Room icons for the sidebar.
export const Home = ({ size }: Size) => (
  <svg {...svg(size)}>
    <path d="M4 11l8-7 8 7" />
    <path d="M6 9.5V20h12V9.5" />
  </svg>
);

export const Dining = ({ size }: Size) => (
  <svg {...svg(size)}>
    <path d="M7 3v8M5 3v4a2 2 0 0 0 4 0V3M7 11v10" />
    <path d="M16 21V3c2 1 3 3.5 3 7h-3" />
  </svg>
);

export const Sofa = ({ size }: Size) => (
  <svg {...svg(size)}>
    <path d="M5 11V8a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v3" />
    <path d="M3 13a2 2 0 0 1 4 0v2h10v-2a2 2 0 0 1 4 0v5H3z" />
    <path d="M6 18v2M18 18v2" />
  </svg>
);

export const Bed = ({ size }: Size) => (
  <svg {...svg(size)}>
    <path d="M3 18V7M3 14h18v4M21 14v-2a3 3 0 0 0-3-3h-7v5" />
    <circle cx="7" cy="11" r="1.6" />
  </svg>
);

export const Cart = ({ size }: Size) => (
  <svg {...svg(size)}>
    <path d="M3 4h2.5l2.2 10.2a1.5 1.5 0 0 0 1.5 1.2h7.9a1.5 1.5 0 0 0 1.5-1.1L20.5 8H6.4" />
    <circle cx="9.5" cy="19.5" r="1.3" />
    <circle cx="17" cy="19.5" r="1.3" />
  </svg>
);

export const Check = ({ size }: Size) => (
  <svg {...svg(size)} stroke-width={2.6}>
    <path d="M5 12.5l4.5 4.5L19 7.5" />
  </svg>
);

export const Plus = ({ size }: Size) => (
  <svg {...svg(size)} stroke-width={2.4}>
    <path d="M12 5v14M5 12h14" />
  </svg>
);

export const ChevronLeft = ({ size }: Size) => (
  <svg {...svg(size)} stroke-width={2.2}>
    <path d="M15 5l-7 7 7 7" />
  </svg>
);

export const ChevronRight = ({ size }: Size) => (
  <svg {...svg(size)} stroke-width={2.2}>
    <path d="M9 5l7 7-7 7" />
  </svg>
);

export const Gear = ({ size }: Size) => (
  <svg {...svg(size)}>
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" />
  </svg>
);

export const Close = ({ size }: Size) => (
  <svg {...svg(size)} stroke-width={2.2}>
    <path d="M6 6l12 12M18 6L6 18" />
  </svg>
);

export const Image = ({ size }: Size) => (
  <svg {...svg(size)}>
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <circle cx="9" cy="10" r="2" />
    <path d="M21 16l-5-5-8 9" />
  </svg>
);

export const ICONS: { [name: string]: (p: Size) => any } = {
  bulb: Bulb, power: Power, fan: Fan, moon: Moon, thermo: Thermo, drop: Drop,
  gauge: Gauge, home: Home, dining: Dining, sofa: Sofa, bed: Bed, cart: Cart,
};
