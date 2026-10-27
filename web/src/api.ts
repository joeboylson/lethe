export type Note = {
  id: number;
  project: string;
  project_path: string;
  path: string;
  title: string;
  session_id: string | null;
  created_at: string;
  status: "waiting" | "picked_up" | "cancelled" | "kept";
  picked_up_by: string | null;
  picked_up_at: string | null;
  archived: boolean;
};

export type Project = { name: string; path: string; count: number; latest: string; waiting: number };

export type Session = {
  id: string;
  cwd?: string | null;
  source?: string | null;
  started_at?: string;
  ended_at?: string | null;
  end_reason?: string | null;
  replaced_by?: string | null;
} | null;

export type NoteDetail = { note: Note; body: string; writer: Session; reader: Session };

async function call<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(path, init);
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export const api = {
  projects: () => call<Project[]>("/api/projects"),
  notes: (project: string | null, archived: boolean) => {
    const q = new URLSearchParams({ archived: String(archived) });
    if (project) q.set("project", project);
    return call<Note[]>(`/api/notes?${q}`);
  },
  note: (id: number) => call<NoteDetail>(`/api/notes/${id}`),
  archive: (id: number, archived: boolean) =>
    call(`/api/notes/${id}/archive`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ archived }),
    }),
};

export function when(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso);
  return d.toLocaleString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", hour12: false });
}

export const short = (id: string | null | undefined) => (id ? id.slice(0, 8) : "");

export const STATUS: Record<Note["status"], string> = {
  waiting: "waiting for /clear",
  picked_up: "picked up",
  cancelled: "not picked up",
  kept: "not picked up",
};
