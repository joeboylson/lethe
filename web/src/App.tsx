import { useCallback, useEffect, useMemo, useState } from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { api, short, STATUS, when, type Note, type NoteDetail, type Project } from "./api";

const POLL_MS = 5000;

export function App() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [project, setProject] = useState<string | null>(null);
  const [archived, setArchived] = useState(false);
  const [notes, setNotes] = useState<Note[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [detail, setDetail] = useState<NoteDetail | null>(null);
  const [filter, setFilter] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [p, n] = await Promise.all([api.projects(), api.notes(project, archived)]);
      setProjects(p);
      setNotes(n);
      setError(null);
    } catch (e) {
      setError(`Can't reach lethe: ${(e as Error).message}`);
    }
  }, [project, archived]);

  useEffect(() => {
    refresh();
    const t = setInterval(refresh, POLL_MS);
    return () => clearInterval(t);
  }, [refresh]);

  // Keep the open note current too: it may get picked up while it's on screen.
  const openNote = useCallback(async (id: number | null) => {
    if (id == null) return setDetail(null);
    try {
      setDetail(await api.note(id));
    } catch (e) {
      setError((e as Error).message);
    }
  }, []);
  useEffect(() => {
    openNote(selected);
    setCopied(false);
  }, [selected, openNote]);
  useEffect(() => {
    if (selected != null) openNote(selected);
    else if (notes.length > 0) setSelected(notes[0].id);
  }, [notes, selected, openNote]);

  const shown = useMemo(() => {
    const f = filter.trim().toLowerCase();
    return f ? notes.filter((n) => `${n.title} ${n.project}`.toLowerCase().includes(f)) : notes;
  }, [notes, filter]);

  const total = projects.reduce((s, p) => s + p.count, 0);
  const waiting = projects.reduce((s, p) => s + p.waiting, 0);
  const projectName = projects.find((p) => p.path === project)?.name;

  async function copyRecall(id: number) {
    await navigator.clipboard.writeText(`/recall ${id}`);
    setCopied(true);
  }

  async function toggleArchive(n: Note) {
    await api.archive(n.id, !n.archived);
    setSelected(null);
    refresh();
  }

  return (
    <div className="wb lethe">
      <header className="wb-titlebar">
        <span className="label brand">lethe</span>
        <input
          className="input"
          type="search"
          placeholder="Filter notes"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          aria-label="Filter notes"
        />
        <label className="check">
          <input type="checkbox" checked={archived} onChange={(e) => { setArchived(e.target.checked); setSelected(null); }} />
          Archived
        </label>
      </header>

      <aside className="wb-sidebar">
        <div className="pane-head"><span className="label">Projects</span></div>
        <ul className="tree">
          <li>
            <button aria-selected={project == null} onClick={() => { setProject(null); setSelected(null); }}>
              All projects <span className="muted">{total}</span>
            </button>
          </li>
          {projects.map((p) => (
            <li key={p.path}>
              <button
                aria-selected={project === p.path}
                title={p.path}
                onClick={() => { setProject(p.path); setSelected(null); }}
              >
                {p.waiting > 0 && <span className="dot waiting" title="A note is waiting for /clear" />}
                <span className="ellipsis">{p.name}</span>
                <span className="muted">{p.count}</span>
              </button>
            </li>
          ))}
        </ul>
      </aside>

      <main className="wb-main lethe-main">
        <section className="list-pane">
          <div className="pane-head">
            <span className="label">{archived ? "Archived" : "Notes"}{projectName ? ` · ${projectName}` : ""}</span>
            <span className="count">{shown.length}</span>
          </div>
          {shown.length === 0 ? (
            <p className="empty">
              {notes.length === 0 ? (archived ? "Nothing archived." : "No notes yet. Run /lethe in a Claude Code session.") : "Nothing matches."}
            </p>
          ) : (
            <ul className="note-list">
              {shown.map((n) => (
                <li key={n.id}>
                  <button aria-selected={selected === n.id} onClick={() => setSelected(n.id)}>
                    <span className="note-title">{n.title}</span>
                    <span className="note-meta">
                      <span>#{n.id}</span>
                      <span>{when(n.created_at)}</span>
                      {project == null && <span>{n.project}</span>}
                      <span className={`status status--${n.status}`}>{STATUS[n.status]}</span>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className="reader">
          {detail ? (
            <>
              <div className="reader-head">
                <div className="reader-title">
                  <h1>{detail.note.title}</h1>
                  <div className="actions">
                    <button className="btn" onClick={() => copyRecall(detail.note.id)}>
                      {copied ? "Copied" : `Copy /recall ${detail.note.id}`}
                    </button>
                    <button className="btn btn--secondary" onClick={() => toggleArchive(detail.note)}>
                      {detail.note.archived ? "Restore" : "Archive"}
                    </button>
                  </div>
                </div>
                <dl className="facts">
                  <dt>Project</dt>
                  <dd title={detail.note.project_path}>{detail.note.project}</dd>
                  <dt>Written</dt>
                  <dd>{when(detail.note.created_at)} by session {short(detail.note.session_id) || "outside Claude"}</dd>
                  <dt>Picked up</dt>
                  <dd>
                    {detail.note.picked_up_by
                      ? `${when(detail.note.picked_up_at)} by session ${short(detail.note.picked_up_by)}${detail.reader?.source === "clear" ? " after /clear" : " with /recall"}`
                      : STATUS[detail.note.status]}
                  </dd>
                  {detail.writer?.replaced_by && (
                    <>
                      <dt>Window</dt>
                      <dd>session {short(detail.writer.id)} was cleared; {short(detail.writer.replaced_by)} took over</dd>
                    </>
                  )}
                  <dt>File</dt>
                  <dd className="path">{detail.note.path}</dd>
                </dl>
              </div>
              <article className="note-body">
                <Markdown remarkPlugins={[remarkGfm]}>{detail.body}</Markdown>
              </article>
            </>
          ) : (
            <p className="empty">Pick a note to read it.</p>
          )}
        </section>
      </main>

      <footer className="wb-status">
        <span>{total} notes</span>
        <span>
          {waiting > 0 && <span className="dot waiting" />}
          {waiting} waiting
        </span>
        <span className="spacer" />
        {error && <span className="error">{error}</span>}
        <span className="hide-sm">{location.host}</span>
      </footer>
    </div>
  );
}
