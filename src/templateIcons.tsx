import {
  Briefcase,
  CalendarRange,
  Compass,
  FileText,
  GraduationCap,
  Handshake,
  HeartHandshake,
  LayoutTemplate,
  Lightbulb,
  Presentation,
  Rocket,
  RotateCcw,
  Scale,
  Timer,
  UserSearch,
  Users,
  Wrench,
} from "lucide-react";
import { ReactNode } from "react";

const ICONS: Record<string, (size: number) => ReactNode> = {
  general: (s) => <FileText size={s} />,
  "one-on-one": (s) => <Users size={s} />,
  "sales-call": (s) => <Handshake size={s} />,
  standup: (s) => <Timer size={s} />,
  "customer-discovery": (s) => <Compass size={s} />,
  "product-demo": (s) => <Presentation size={s} />,
  "customer-check-in": (s) => <HeartHandshake size={s} />,
  interview: (s) => <UserSearch size={s} />,
  "project-kickoff": (s) => <Rocket size={s} />,
  retrospective: (s) => <RotateCcw size={s} />,
  "weekly-team": (s) => <CalendarRange size={s} />,
  "exec-update": (s) => <Briefcase size={s} />,
  brainstorm: (s) => <Lightbulb size={s} />,
  troubleshooting: (s) => <Wrench size={s} />,
  "partner-negotiation": (s) => <Scale size={s} />,
  learning: (s) => <GraduationCap size={s} />,
};

/** The icon for a built-in template; custom templates share one. */
export const templateIcon = (id: string, size = 16) => (ICONS[id] ?? ((s: number) => <LayoutTemplate size={s} />))(size);
