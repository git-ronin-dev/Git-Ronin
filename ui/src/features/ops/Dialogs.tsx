import { useQuery } from "@tanstack/react-query";
import { open as pickFolder } from "@tauri-apps/plugin-dialog";
import { useState } from "react";

import type { FlowConfig } from "../../bindings/FlowConfig";
import type { FlowKind } from "../../bindings/FlowKind";
import { ipc } from "../../lib/ipc";
import { Button } from "../../ui/Button";
import { Checkbox, Field, Select, TextInput } from "../../ui/Field";
import { toast } from "../../ui/toast-store";
import { useConfig, useRefs, useRepoInfo } from "../workspace/queries";
import { useWorkspace } from "../workspace/store";
import { CreateIssue, CreatePullRequest } from "../hosting/HostingDialogs";
import { RepoPicker } from "../hosting/RepoPicker";
import { useGitActions } from "./actions";
import { FormDialog } from "./FormDialog";
import { useDialogs, type DialogRequest } from "./dialog-store";
import { branchPrefixes } from "./prefixes";
import { useFlowConfig } from "./queries";
import { startTask, useTask } from "./tasks";

type Of<K extends DialogRequest["kind"]> = Extract<DialogRequest, { kind: K }>;

/** Hosts the dialog opened with `openDialog()`. Mounted once, in App. */
export function Dialogs() {
  const { request, close } = useDialogs();
  if (!request) return null;
  // Keyed so each request starts with fresh fields.
  const key = JSON.stringify(request);
  switch (request.kind) {
    case "createBranch":
      return <CreateBranch key={key} {...request} onClose={close} />;
    case "renameBranch":
      return <RenameBranch key={key} {...request} onClose={close} />;
    case "createTag":
      return <CreateTag key={key} {...request} onClose={close} />;
    case "remote":
      return <RemoteDialog key={key} {...request} onClose={close} />;
    case "pushNew":
      return <PushNew key={key} {...request} onClose={close} />;
    case "upstream":
      return <UpstreamDialog key={key} {...request} onClose={close} />;
    case "clone":
      return <CloneDialog key={key} onClose={close} />;
    case "addWorktree":
      return <AddWorktree key={key} {...request} onClose={close} />;
    case "addSubmodule":
      return <AddSubmodule key={key} {...request} onClose={close} />;
    case "lfsTrack":
      return <LfsTrack key={key} {...request} onClose={close} />;
    case "flowInit":
      return <FlowInit key={key} {...request} onClose={close} />;
    case "flowStart":
      return <FlowStart key={key} {...request} onClose={close} />;
    case "flowFinish":
      return <FlowFinish key={key} {...request} onClose={close} />;
    case "createPullRequest":
      return <CreatePullRequest key={key} {...request} onClose={close} />;
    case "createIssue":
      return <CreateIssue key={key} {...request} onClose={close} />;
  }
}

function CreateBranch({
  repo,
  start,
  startLabel,
  onClose,
}: Of<"createBranch"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const [name, setName] = useState("");
  const [checkout, setCheckout] = useState(true);
  const prefixes = branchPrefixes(useRefs(repo).data, useFlowConfig(repo).data ?? null);
  return (
    <FormDialog
      title="Create branch"
      submitLabel="Create"
      blocker={name.trim() ? null : "Enter a name"}
      onSubmit={() => actions.createBranch(name.trim(), start, checkout)}
      onClose={onClose}
    >
      <Field label="Name" hint={`Starts at ${startLabel}`}>
        <TextInput value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </Field>
      {prefixes.length > 0 && (
        <PrefixChips
          prefixes={prefixes}
          name={name}
          onPick={(prefix) => setName(withPrefix(name, prefix, prefixes))}
        />
      )}
      <Checkbox label="Check it out" checked={checkout} onChange={setCheckout} />
    </FormDialog>
  );
}

function RenameBranch({ repo, name, onClose }: Of<"renameBranch"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const [next, setNext] = useState(name);
  return (
    <FormDialog
      title={`Rename ${name}`}
      submitLabel="Rename"
      blocker={!next.trim() || next.trim() === name ? "Enter a new name" : null}
      onSubmit={() => actions.renameBranch(name, next.trim())}
      onClose={onClose}
    >
      <Field label="New name">
        <TextInput
          value={next}
          onChange={(e) => setNext(e.target.value)}
          onFocus={(e) => e.target.select()}
          autoFocus
        />
      </Field>
    </FormDialog>
  );
}

function CreateTag({
  repo,
  target,
  targetLabel,
  onClose,
}: Of<"createTag"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const [name, setName] = useState("");
  const [message, setMessage] = useState("");
  return (
    <FormDialog
      title="Create tag"
      submitLabel="Create"
      blocker={name.trim() ? null : "Enter a name"}
      onSubmit={() => actions.createTag(name.trim(), target, message.trim() || null)}
      onClose={onClose}
    >
      <Field label="Name" hint={`Tags ${targetLabel}`}>
        <TextInput value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </Field>
      <Field
        label="Message (optional)"
        hint="With a message the tag is annotated: it records who tagged, and when."
      >
        <textarea
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          rows={3}
          className="w-full resize-none rounded-md border border-line bg-canvas px-2 py-1 text-fg outline-none focus:border-accent"
        />
      </Field>
    </FormDialog>
  );
}

function RemoteDialog({ repo, remote, onClose }: Of<"remote"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const [name, setName] = useState(remote?.name ?? "");
  const [url, setUrl] = useState(remote?.url ?? "");
  const [pushUrl, setPushUrl] = useState(remote?.pushUrl ?? "");
  const [fetchNow, setFetchNow] = useState(true);
  const submit = async () => {
    if (remote) {
      return actions.editRemote(remote.name, name.trim(), url.trim(), pushUrl.trim() || null);
    }
    const added = await actions.addRemote(name.trim(), url.trim());
    if (added && fetchNow) void actions.fetch(name.trim());
    return added;
  };
  return (
    <FormDialog
      title={remote ? `Edit ${remote.name}` : "Add remote"}
      submitLabel={remote ? "Save" : "Add"}
      blocker={!name.trim() ? "Enter a name" : !url.trim() ? "Enter a URL" : null}
      onSubmit={submit}
      onClose={onClose}
    >
      <Field label="Name">
        <TextInput
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="origin"
          autoFocus={!remote}
        />
      </Field>
      <Field label="URL">
        <TextInput
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="https://github.com/owner/repo.git"
        />
      </Field>
      {remote ? (
        <Field label="Push URL (optional)" hint="Leave empty to push to the URL above.">
          <TextInput value={pushUrl} onChange={(e) => setPushUrl(e.target.value)} />
        </Field>
      ) : (
        <Checkbox label="Fetch it now" checked={fetchNow} onChange={setFetchNow} />
      )}
    </FormDialog>
  );
}

function PushNew({ repo, branch, onClose }: Of<"pushNew"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const remotes = useRefs(repo).data?.remotes ?? [];
  const [remote, setRemote] = useState(remotes[0]?.name ?? "");
  const [name, setName] = useState(branch);
  const chosen = remote || remotes[0]?.name || "";
  return (
    <FormDialog
      title={`Push ${branch}`}
      submitLabel="Push"
      blocker={!chosen ? "Add a remote first" : !name.trim() ? "Enter a branch name" : null}
      onSubmit={() => actions.pushTo(branch, { remote: chosen, branch: name.trim() })}
      onClose={onClose}
    >
      <p>
        {branch} has no upstream yet. Where should it go? It will track that branch from now on.
      </p>
      <Field label="Remote">
        <Select value={chosen} onChange={(e) => setRemote(e.target.value)}>
          {remotes.map((r) => (
            <option key={r.name} value={r.name}>
              {r.name}
            </option>
          ))}
        </Select>
      </Field>
      <Field label="Branch on the remote">
        <TextInput value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </Field>
    </FormDialog>
  );
}

function UpstreamDialog({
  repo,
  branch,
  upstream,
  onClose,
}: Of<"upstream"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const remotes = useRefs(repo).data?.remotes ?? [];
  const [value, setValue] = useState(upstream ?? "");
  const options = remotes.flatMap((r) => r.branches.map((b) => `${r.name}/${b.name}`));
  return (
    <FormDialog
      title={`Upstream of ${branch}`}
      submitLabel="Save"
      blocker={value === (upstream ?? "") ? "Pick a different branch" : null}
      onSubmit={() => actions.setUpstream(branch, value || null)}
      onClose={onClose}
    >
      <Field
        label="Tracks"
        hint="Pull and push use this branch; ahead/behind counts compare with it."
      >
        <Select value={value} onChange={(e) => setValue(e.target.value)} autoFocus>
          <option value="">None</option>
          {options.map((o) => (
            <option key={o} value={o}>
              {o}
            </option>
          ))}
        </Select>
      </Field>
    </FormDialog>
  );
}

/** The folder containing `path`, or "". */
function parentOf(path: string | null | undefined): string {
  if (!path) return "";
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut > 0 ? path.slice(0, cut) : "";
}

function CloneDialog({ onClose }: { onClose: () => void }) {
  const recent = useConfig().data?.local.recentRepos;
  const active = useWorkspace((s) => s.active);
  const [url, setUrl] = useState("");
  // Next to the repository in view, else the most recently opened one.
  const [parent, setParent] = useState(() => parentOf(active ?? recent?.[0]));
  const [name, setName] = useState<string | null>(null);
  const suggested = useQuery({
    queryKey: ["cloneName", url.trim()],
    queryFn: () => ipc.cloneName(url.trim()),
    enabled: url.trim().length > 0,
  }).data;
  const folder = name ?? suggested ?? "";
  const separator = parent.includes("\\") && !parent.includes("/") ? "\\" : "/";
  const dest = parent && folder ? `${parent.replace(/[\\/]+$/, "")}${separator}${folder}` : "";
  const task = useTask(dest);

  const browse = async () => {
    const picked = await pickFolder({
      directory: true,
      title: "Clone into",
      defaultPath: parent || undefined,
    });
    if (picked) setParent(picked);
  };
  const submit = async () => {
    try {
      const info = await ipc.cloneRepo(url.trim(), parent, folder);
      useWorkspace.getState().adopt(info);
      toast.success(`Cloned ${info.name}`);
      return true;
    } catch (err) {
      toast.error("Could not clone", String(err));
      return false;
    }
  };

  return (
    <FormDialog
      title="Clone repository"
      submitLabel={task ? "Cloning…" : "Clone"}
      blocker={
        !url.trim() ? "Enter a URL" : !parent ? "Choose a folder" : !folder ? "Enter a name" : null
      }
      onSubmit={async () => {
        const done = startTask(dest, "Cloning");
        try {
          return await submit();
        } finally {
          done();
        }
      }}
      onClose={onClose}
    >
      <RepoPicker url={url.trim()} onPick={setUrl} />
      <Field label="URL">
        <TextInput
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="https://github.com/owner/repo.git"
          autoFocus
        />
      </Field>
      <Field label="Into folder">
        <div className="flex gap-2">
          <TextInput value={parent} onChange={(e) => setParent(e.target.value)} />
          <Button type="button" onClick={() => void browse()}>
            Browse…
          </Button>
        </div>
      </Field>
      <Field label="Name" hint={dest && `Creates ${dest}`}>
        <TextInput value={folder} onChange={(e) => setName(e.target.value)} />
      </Field>
      {task && (
        <div className="space-y-1" aria-live="polite">
          <div className="h-1 overflow-hidden rounded-full bg-raised">
            <div
              className="h-full bg-accent transition-[width]"
              style={{ width: `${task.percent ?? 0}%` }}
            />
          </div>
          <p className="truncate text-xs text-fg-faint">{task.message ?? "Starting…"}</p>
        </div>
      )}
    </FormDialog>
  );
}

/** Replaces `name`'s prefix (one of `prefixes`, if it has one) with `prefix`. */
function withPrefix(name: string, prefix: string, prefixes: string[]): string {
  const current = prefixes.find((p) => name.startsWith(p));
  const rest = current ? name.slice(current.length) : name;
  return current === prefix ? rest : prefix + rest;
}

function PrefixChips({
  prefixes,
  name,
  onPick,
}: {
  prefixes: string[];
  name: string;
  onPick: (prefix: string) => void;
}) {
  return (
    <div className="flex flex-wrap items-center gap-1.5" role="group" aria-label="Prefixes">
      {prefixes.map((p) => (
        <button
          key={p}
          type="button"
          aria-pressed={name.startsWith(p)}
          onClick={() => onPick(p)}
          className={
            name.startsWith(p)
              ? "rounded-sm border border-accent bg-accent/20 px-1.5 text-xs text-fg"
              : "rounded-sm border border-line px-1.5 text-xs text-fg-muted hover:text-fg"
          }
        >
          {p}
        </button>
      ))}
    </div>
  );
}

/** A folder next to the repository, e.g. `/work/app` + `topic` gives `/work/app-topic`. */
function siblingFolder(repo: string, branch: string): string {
  const separator = repo.includes("\\") && !repo.includes("/") ? "\\" : "/";
  const safe = branch.replace(/[\\/:*?"<>|]+/g, "-");
  return `${repo.replace(/[\\/]+$/, "")}-${safe}`.replace(/[\\/]/g, separator);
}

function AddWorktree({ repo, branch, onClose }: Of<"addWorktree"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const refs = useRefs(repo).data;
  const available = (refs?.local ?? []).filter((b) => !b.isHead && !b.worktree).map((b) => b.name);
  const [create, setCreate] = useState(branch === null);
  const [name, setName] = useState(branch ?? "");
  const [existing, setExisting] = useState(branch ?? available[0] ?? "");
  const chosen = create ? name.trim() : existing;
  const [dest, setDest] = useState<string | null>(null);
  const folder = dest ?? (chosen ? siblingFolder(repo, chosen) : "");

  const browse = async () => {
    const picked = await pickFolder({ directory: true, title: "Working tree folder" });
    if (picked) setDest(picked);
  };
  const submit = async () => {
    const ok = await actions.addWorktree(folder, chosen, create, null);
    if (ok) toast.success("Created the working tree", folder);
    return ok;
  };
  return (
    <FormDialog
      title="Add working tree"
      submitLabel="Create"
      blocker={!chosen ? "Choose a branch" : !folder ? "Choose a folder" : null}
      onSubmit={submit}
      onClose={onClose}
    >
      <p>A second folder with its own checkout of this repository, sharing its history.</p>
      <Checkbox label="Create a new branch (from HEAD)" checked={create} onChange={setCreate} />
      {create ? (
        <Field label="New branch">
          <TextInput value={name} onChange={(e) => setName(e.target.value)} autoFocus />
        </Field>
      ) : (
        <Field label="Branch" hint="Branches checked out elsewhere can't be used.">
          <Select value={existing} onChange={(e) => setExisting(e.target.value)}>
            {available.map((b) => (
              <option key={b} value={b}>
                {b}
              </option>
            ))}
          </Select>
        </Field>
      )}
      <Field label="Folder" hint="Created if it doesn't exist; must be empty if it does.">
        <div className="flex gap-2">
          <TextInput value={folder} onChange={(e) => setDest(e.target.value)} />
          <Button type="button" onClick={() => void browse()}>
            Browse…
          </Button>
        </div>
      </Field>
    </FormDialog>
  );
}

function AddSubmodule({ repo, onClose }: Of<"addSubmodule"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const [url, setUrl] = useState("");
  const [path, setPath] = useState<string | null>(null);
  const suggested = useQuery({
    queryKey: ["cloneName", url.trim()],
    queryFn: () => ipc.cloneName(url.trim()),
    enabled: url.trim().length > 0,
  }).data;
  const folder = path ?? suggested ?? "";
  const submit = async () => {
    const ok = await actions.addSubmodule(url.trim(), folder.trim());
    if (ok) toast.success(`Added submodule ${folder}`, "It is staged, ready to commit.");
    return ok;
  };
  return (
    <FormDialog
      title="Add submodule"
      submitLabel="Add"
      blocker={!url.trim() ? "Enter a URL" : !folder.trim() ? "Enter a path" : null}
      onSubmit={submit}
      onClose={onClose}
    >
      <Field label="URL">
        <TextInput
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="https://github.com/owner/library.git"
          autoFocus
        />
      </Field>
      <Field label="Path" hint="Inside this repository, e.g. vendor/library.">
        <TextInput value={folder} onChange={(e) => setPath(e.target.value)} />
      </Field>
    </FormDialog>
  );
}

function LfsTrack({ repo, pattern, onClose }: Of<"lfsTrack"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const [value, setValue] = useState(pattern);
  const submit = async () => {
    const ok = await actions.lfsTrack(value.trim());
    if (ok)
      toast.success(`Tracking ${value.trim()} with LFS`, "Commit .gitattributes to share it.");
    return ok;
  };
  return (
    <FormDialog
      title="Track files with Git LFS"
      submitLabel="Track"
      blocker={value.trim() ? null : "Enter a pattern"}
      onSubmit={submit}
      onClose={onClose}
    >
      <Field
        label="Pattern"
        hint="Matching files are stored in LFS from now on; files already committed stay as they are."
      >
        <TextInput
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder="*.psd"
          autoFocus
        />
      </Field>
    </FormDialog>
  );
}

function FlowInit({ repo, onClose }: Of<"flowInit"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const refs = useRefs(repo).data;
  const names = refs?.local.map((b) => b.name) ?? [];
  const [config, setConfig] = useState<FlowConfig>(() => ({
    master: ["main", "master"].find((n) => names.includes(n)) ?? names[0] ?? "main",
    develop: "develop",
    featurePrefix: "feature/",
    releasePrefix: "release/",
    hotfixPrefix: "hotfix/",
    versionTagPrefix: "",
  }));
  const set =
    (key: keyof FlowConfig) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) =>
      setConfig((c) => ({ ...c, [key]: e.target.value }));
  return (
    <FormDialog
      title="Set up Git Flow"
      submitLabel="Set up"
      blocker={
        !config.master || !config.develop.trim()
          ? "Name both branches"
          : config.master === config.develop.trim()
            ? "Use two different branches"
            : null
      }
      onSubmit={() => actions.flowInit({ ...config, develop: config.develop.trim() })}
      onClose={onClose}
    >
      <div className="grid grid-cols-2 gap-3">
        <Field label="Production branch">
          <Select value={config.master} onChange={set("master")}>
            {names.map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </Select>
        </Field>
        <Field label="Development branch" hint="Created from production if missing.">
          <TextInput value={config.develop} onChange={set("develop")} />
        </Field>
        <Field label="Feature prefix">
          <TextInput value={config.featurePrefix} onChange={set("featurePrefix")} />
        </Field>
        <Field label="Release prefix">
          <TextInput value={config.releasePrefix} onChange={set("releasePrefix")} />
        </Field>
        <Field label="Hotfix prefix">
          <TextInput value={config.hotfixPrefix} onChange={set("hotfixPrefix")} />
        </Field>
        <Field label="Version tag prefix">
          <TextInput
            value={config.versionTagPrefix}
            onChange={set("versionTagPrefix")}
            placeholder="e.g. v"
          />
        </Field>
      </div>
    </FormDialog>
  );
}

const FLOW_BASE: Record<FlowKind, "develop" | "master"> = {
  feature: "develop",
  release: "develop",
  hotfix: "master",
};

function FlowStart({ repo, flow, onClose }: Of<"flowStart"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const config = useFlowConfig(repo).data;
  const [name, setName] = useState("");
  if (!config) return null;
  const prefix = {
    feature: config.featurePrefix,
    release: config.releasePrefix,
    hotfix: config.hotfixPrefix,
  }[flow];
  const base = config[FLOW_BASE[flow]];
  return (
    <FormDialog
      title={`Start ${flow}`}
      submitLabel="Start"
      blocker={name.trim() ? null : flow === "feature" ? "Enter a name" : "Enter a version"}
      onSubmit={() => actions.flowStart(flow, name.trim())}
      onClose={onClose}
    >
      <Field
        label={flow === "feature" ? "Name" : "Version"}
        hint={`Creates ${prefix}${name.trim() || "…"} from ${base} and checks it out.`}
      >
        <TextInput value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </Field>
    </FormDialog>
  );
}

function FlowFinish({ repo, flow, branch, onClose }: Of<"flowFinish"> & { onClose: () => void }) {
  const actions = useGitActions(repo);
  const config = useFlowConfig(repo).data;
  const info = useRepoInfo(repo).data;
  const [message, setMessage] = useState("");
  const [keep, setKeep] = useState(false);
  if (!config) return null;
  const prefix = {
    feature: config.featurePrefix,
    release: config.releasePrefix,
    hotfix: config.hotfixPrefix,
  }[flow];
  const version = branch.slice(prefix.length);
  const tag = `${config.versionTagPrefix}${version}`;
  const into = flow === "feature" ? config.develop : `${config.master}, then ${config.develop}`;
  return (
    <FormDialog
      title={`Finish ${branch}`}
      submitLabel="Finish"
      blocker={info?.operation ? "Finish the operation in progress first" : null}
      onSubmit={() => actions.flowFinish(flow, branch, message.trim() || null, keep)}
      onClose={onClose}
    >
      <p>
        Merges {branch} into {into}
        {flow !== "feature" && `, tagging ${config.master} as ${tag}`}. Uncommitted changes must not
        be in the way. If a merge stops on conflicts, resolve them, commit, and finish again.
      </p>
      {flow !== "feature" && (
        <Field label="Tag message (optional)">
          <TextInput
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            placeholder={`${flow === "hotfix" ? "Hotfix" : "Release"} ${version}`}
          />
        </Field>
      )}
      <Checkbox label={`Keep ${branch} afterwards`} checked={keep} onChange={setKeep} />
    </FormDialog>
  );
}
