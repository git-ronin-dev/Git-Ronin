/*
 * The app's own icon set: 24×24 strokes in `currentColor`, sized with
 * classes (`size-4`). Git nodes and status circles are ensō, circles drawn
 * in one stroke with a gap, like the commit dots in the graph.
 */
import type { ComponentType, ReactNode, SVGProps } from "react";

import { ensoPath } from "./enso";

export type IconProps = SVGProps<SVGSVGElement>;
export type Icon = ComponentType<IconProps>;

function make(name: string, body: ReactNode): Icon {
  function Glyph({ strokeWidth = 1.75, ...props }: IconProps) {
    return (
      <svg
        xmlns="http://www.w3.org/2000/svg"
        width={24}
        height={24}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={strokeWidth}
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden={props["aria-label"] ? undefined : true}
        role={props["aria-label"] ? "img" : undefined}
        {...props}
      >
        {body}
      </svg>
    );
  }
  Glyph.displayName = name;
  return Glyph;
}

const enso = (cx: number, cy: number, r: number, gap?: number) => (
  <path d={ensoPath(cx, cy, r, gap)} />
);
/** A git node: a small ensō. */
const node = (cx: number, cy: number) => enso(cx, cy, 2.6, 70);

// Navigation and editing

export const X = make("X", <path d="M6.5 6.5l11 11M17.5 6.5l-11 11" />);
export const Plus = make("Plus", <path d="M12 5v14M5 12h14" />);
export const Minus = make("Minus", <path d="M5 12h14" />);
export const Check = make("Check", <path d="M4.5 12.5l4.8 4.8L19.5 7" />);
export const ChevronRight = make("ChevronRight", <path d="M9.5 6l6 6-6 6" />);
export const ChevronDown = make("ChevronDown", <path d="M6 9.5l6 6 6-6" />);
export const ChevronUp = make("ChevronUp", <path d="M6 14.5l6-6 6 6" />);
export const ArrowLeft = make("ArrowLeft", <path d="M19 12H5.5M11 6l-6 6 6 6" />);
export const ArrowUp = make("ArrowUp", <path d="M12 19V5.5M6 11l6-6 6 6" />);
export const ArrowDown = make("ArrowDown", <path d="M12 5v13.5M6 13l6 6 6-6" />);
export const ArrowDownToLine = make(
  "ArrowDownToLine",
  <path d="M12 3.5v12M7 10.5l5 5 5-5M5 20h14" />,
);
export const ArrowUpFromLine = make(
  "ArrowUpFromLine",
  <path d="M12 16.5v-12M7 9.5l5-5 5 5M5 20h14" />,
);
export const ArrowDownCircle = make(
  "ArrowDownCircle",
  <>
    {enso(12, 12, 9)}
    <path d="M12 7.5v8M8.5 12.5l3.5 3.5 3.5-3.5" />
  </>,
);
export const CornerLeftDown = make(
  "CornerLeftDown",
  <path d="M19 4.5h-6a4 4 0 0 0-4 4v11M5 15.5l4 4 4-4" />,
);
export const Undo2 = make(
  "Undo2",
  <path d="M9 14.5L4.5 10 9 5.5M4.5 10H14a5.5 5.5 0 0 1 0 11h-3" />,
);
export const Redo2 = make(
  "Redo2",
  <path d="M15 14.5l4.5-4.5L15 5.5M19.5 10H10a5.5 5.5 0 0 0 0 11h3" />,
);
export const RefreshCw = make(
  "RefreshCw",
  <path d="M19.5 9.5A8 8 0 0 0 5.2 7.4M4.5 14.5a8 8 0 0 0 14.3 2.1M5 3.5v4h4M19 20.5v-4h-4" />,
);
export const ExternalLink = make(
  "ExternalLink",
  <path d="M13.5 4.5h6v6M19.5 4.5l-8.5 8.5M17.5 14v4.5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1v-11a1 1 0 0 1 1-1H10" />,
);
export const Copy = make(
  "Copy",
  <>
    <rect x="8.5" y="8.5" width="12" height="12" rx="2" />
    <path d="M15.5 5.5v-1a1 1 0 0 0-1-1h-10a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h1" />
  </>,
);
export const Download = make(
  "Download",
  <path d="M12 3.5v11M7.5 10l4.5 4.5 4.5-4.5M4 15.5v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" />,
);
export const Upload = make(
  "Upload",
  <path d="M12 15V4M7.5 8.5L12 4l4.5 4.5M4 15.5v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" />,
);
export const LogIn = make(
  "LogIn",
  <path d="M14 4h4.5a1.5 1.5 0 0 1 1.5 1.5v13a1.5 1.5 0 0 1-1.5 1.5H14M3.5 12H14M10 8l4 4-4 4" />,
);
export const GripVertical = make(
  "GripVertical",
  <path strokeWidth={3} d="M9 6h.01M15 6h.01M9 12h.01M15 12h.01M9 18h.01M15 18h.01" />,
);
export const List = make(
  "List",
  <path d="M9 6h11M9 12h11M9 18h11M4.5 6h.01M4.5 12h.01M4.5 18h.01" />,
);
export const ListTree = make(
  "ListTree",
  <path d="M11 6h9M15 12h5M15 18h5M5 4v12a2 2 0 0 0 2 2h4M5 10a2 2 0 0 0 2 2h4" />,
);

// Status: ensō with a mark inside

export const CircleCheck = make(
  "CircleCheck",
  <>
    {enso(12, 12, 9)}
    <path d="M8.2 12.3l2.6 2.6 5-5.2" />
  </>,
);
export const CircleX = make(
  "CircleX",
  <>
    {enso(12, 12, 9)}
    <path d="M9 9l6 6M15 9l-6 6" />
  </>,
);
export const CircleMinus = make(
  "CircleMinus",
  <>
    {enso(12, 12, 9)}
    <path d="M8 12h8" />
  </>,
);
export const CircleDot = make(
  "CircleDot",
  <>
    {enso(12, 12, 9)}
    <circle cx="12" cy="12" r="1.6" fill="currentColor" />
  </>,
);
export const CircleDashed = make(
  "CircleDashed",
  <circle cx="12" cy="12" r="9" strokeDasharray="3.2 3.9" />,
);
/** Spun with `animate-spin`: a brush stroke three quarters round. */
export const LoaderCircle = make("LoaderCircle", enso(12, 12, 9, 100));
export const TriangleAlert = make(
  "TriangleAlert",
  <path d="M10.3 4.2a2 2 0 0 1 3.4 0l7.4 12.9A2 2 0 0 1 19.4 20H4.6a2 2 0 0 1-1.7-2.9zM12 9.5v4M12 16.8h.01" />,
);

// Git

export const GitBranch = make(
  "GitBranch",
  <>
    {node(6.5, 18)}
    {node(17.5, 6)}
    <path d="M6.5 3v12.4M17.5 8.6c0 5-11 3.2-11 6.8" />
  </>,
);
export const GitBranchPlus = make(
  "GitBranchPlus",
  <>
    {node(6.5, 18)}
    <path d="M6.5 3v12.4M17.5 9c0 5-11 3-11 6.4M17.5 2.5v6M14.5 5.5h6" />
  </>,
);
export const GitMerge = make(
  "GitMerge",
  <>
    {node(6.5, 5.5)}
    {node(6.5, 18.5)}
    {node(18, 13)}
    <path d="M6.5 8.1v7.8M6.5 8.1c0 3.4 3.5 4.9 8.9 4.9" />
  </>,
);
export const GitPullRequest = make(
  "GitPullRequest",
  <>
    {node(6, 5.5)}
    {node(6, 18.5)}
    {node(18, 18.5)}
    <path d="M6 8.1v7.8M18 15.9V9.5a3 3 0 0 0-3-3h-3.5M13.5 4l-2.5 2.5 2.5 2.5" />
  </>,
);
export const Workflow = make(
  "Workflow",
  <>
    {node(5.5, 5.5)}
    {node(18.5, 12)}
    {node(5.5, 18.5)}
    <path d="M8.1 5.5c5.5 0 7.8 1.6 7.8 6.5M15.9 12c0 4.9-2.3 6.5-7.8 6.5" />
  </>,
);
export const Tag = make(
  "Tag",
  <>
    <path d="M3.5 12.1V4.5a1 1 0 0 1 1-1h7.6a2 2 0 0 1 1.4.6l7.3 7.3a2 2 0 0 1 0 2.8l-6.4 6.4a2 2 0 0 1-2.8 0L4.1 13.5a2 2 0 0 1-.6-1.4z" />
    <circle cx="8.3" cy="8.3" r="1.3" fill="currentColor" />
  </>,
);
/** A stash: a lacquered box with its lid. */
export const Archive = make(
  "Archive",
  <path d="M3.5 5.5a1 1 0 0 1 1-1h15a1 1 0 0 1 1 1v3h-17zM5 8.5v10a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-10M10 12.5h4" />,
);
export const ArchiveRestore = make(
  "ArchiveRestore",
  <path d="M3.5 5.5a1 1 0 0 1 1-1h15a1 1 0 0 1 1 1v3h-17zM5 8.5v10a1 1 0 0 0 1 1h3M19 8.5v10a1 1 0 0 1-1 1h-3M12 20v-7.5M9 15.5l3-3 3 3" />,
);
export const Boxes = make(
  "Boxes",
  <>
    <rect x="3.5" y="12.5" width="7.5" height="7.5" rx="1" />
    <rect x="13" y="12.5" width="7.5" height="7.5" rx="1" />
    <rect x="8.25" y="3.5" width="7.5" height="7.5" rx="1" />
  </>,
);
export const HardDriveDownload = make(
  "HardDriveDownload",
  <path d="M12 2.5v8M8.5 7.5l3.5 3.5 3.5-3.5M4 14.5h16a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-4a1 1 0 0 1 1-1zM7 17.5h.01M10.5 17.5h.01" />,
);
export const Lock = make(
  "Lock",
  <>
    <rect x="4.5" y="10.5" width="15" height="10" rx="1.5" />
    <path d="M8 10.5V7.5a4 4 0 0 1 8 0v3M12 14.5v2" />
  </>,
);
export const KeyRound = make(
  "KeyRound",
  <>
    {enso(8, 15.5, 4.5, 40)}
    <path d="M11.2 12.3L20 3.5M16.5 7l2.5 2.5M14 9.5l2 2" />
  </>,
);
export const Cloud = make(
  "Cloud",
  <path d="M7 18.5h10.5a4 4 0 0 0 .6-7.95 6 6 0 0 0-11.6 1.1A3.5 3.5 0 0 0 7 18.5z" />,
);
export const Laptop = make(
  "Laptop",
  <path d="M5 16.5V6.5a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v10M2.5 16.5h19l-1 2.5a1 1 0 0 1-.9.5H4.4a1 1 0 0 1-.9-.5z" />,
);

// Files

export const Folder = make(
  "Folder",
  <path d="M3.5 6.5a1 1 0 0 1 1-1h4.8l2 2.5h8.2a1 1 0 0 1 1 1v9.5a1 1 0 0 1-1 1h-15a1 1 0 0 1-1-1z" />,
);
export const FolderOpen = make(
  "FolderOpen",
  <path d="M3.5 17.5v-11a1 1 0 0 1 1-1h4.8l2 2.5h7.2a1 1 0 0 1 1 1V10M3.5 17.5l2.4-6.2a1.2 1.2 0 0 1 1.1-.8h13.4a.8.8 0 0 1 .75 1.1l-2.3 6.1a1 1 0 0 1-.95.8H4.5a1 1 0 0 1-1-1z" />,
);
export const FolderPlus = make(
  "FolderPlus",
  <path d="M3.5 6.5a1 1 0 0 1 1-1h4.8l2 2.5h8.2a1 1 0 0 1 1 1v9.5a1 1 0 0 1-1 1h-15a1 1 0 0 1-1-1zM12 11v6M9 14h6" />,
);
export const FolderGit2 = make(
  "FolderGit2",
  <>
    <path d="M10 19.5H4.5a1 1 0 0 1-1-1v-12a1 1 0 0 1 1-1h4.8l2 2.5h8.2a1 1 0 0 1 1 1v2.5" />
    {node(15, 13.5)}
    {node(19.5, 19)}
    <path d="M15 16.1v4.4M17.4 13.9c1.4.6 2.1 1.6 2.1 2.5" />
  </>,
);

// Chrome

export const Search = make(
  "Search",
  <>
    {enso(10.5, 10.5, 6.5, 30)}
    <path d="M15.3 15.3L20.5 20.5" />
  </>,
);
export const SearchCheck = make(
  "SearchCheck",
  <>
    {enso(10.5, 10.5, 6.5, 30)}
    <path d="M15.3 15.3L20.5 20.5M7.8 10.7l2 2 3.4-3.6" />
  </>,
);
export const Settings = make(
  "Settings",
  <>
    <path d="M10.3 3.5h3.4l.5 2.4 1.7 1 2.3-.8 1.7 2.9-1.8 1.6v2l1.8 1.6-1.7 2.9-2.3-.8-1.7 1-.5 2.4h-3.4l-.5-2.4-1.7-1-2.3.8-1.7-2.9 1.8-1.6v-2L4 9l1.7-2.9 2.3.8 1.7-1z" />
    {enso(12, 12, 2.6, 70)}
  </>,
);
export const PanelLeft = make(
  "PanelLeft",
  <>
    <rect x="3.5" y="4.5" width="17" height="15" rx="1.5" />
    <path d="M9.5 4.5v15" />
  </>,
);
export const PanelRight = make(
  "PanelRight",
  <>
    <rect x="3.5" y="4.5" width="17" height="15" rx="1.5" />
    <path d="M14.5 4.5v15" />
  </>,
);
export const SquareTerminal = make(
  "SquareTerminal",
  <>
    <rect x="3.5" y="4.5" width="17" height="15" rx="1.5" />
    <path d="M7.5 9.5l3 2.5-3 2.5M12.5 15h4" />
  </>,
);
export const Eye = make(
  "Eye",
  <>
    <path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z" />
    {enso(12, 12, 3, 60)}
  </>,
);
export const EyeOff = make(
  "EyeOff",
  <path d="M9.9 5.8A9 9 0 0 1 12 5.5c6 0 9.5 6.5 9.5 6.5a17 17 0 0 1-2.4 3.2M14.1 14.1a3 3 0 0 1-4.2-4.2M6.6 6.6C3.9 8.3 2.5 12 2.5 12S6 18.5 12 18.5a9 9 0 0 0 5.4-1.9M3.5 3.5l17 17" />,
);
/** The profile menu: a figure under a ronin's straw hat. */
export const UserRound = make(
  "UserRound",
  <path d="M3.5 9.5L12 3.5l8.5 6zM8.5 9.5v1.5a3.5 3.5 0 0 0 7 0V9.5M5 20.5a7 7 0 0 1 14 0" />,
);
/** The launchpad: a paper plane taking off. */
export const PaperPlane = make("PaperPlane", <path d="M21 3.5L3 10.5l7 3 3 7zM21 3.5L10 13.5" />);
