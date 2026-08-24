import { useState } from "react";
import type { CheckinHeaderEntry } from "../types";

interface HeaderRowProps {
  entry: CheckinHeaderEntry;
  actionLabel?: string;
  onAction?: () => void;
}

export default function HeaderRow({ entry, actionLabel, onAction }: HeaderRowProps) {
  const [noteOpen, setNoteOpen] = useState(false);
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(entry.value);
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    } catch {}
  };

  return (
    <div className="hdr-row">
      <div className="hdr-row-main">
        <span className="hdr-key">{entry.name}</span>
        <span className="hdr-colon">:</span>
        <span className="hdr-val" title={entry.value}>{entry.value}</span>
        {/* ⚠️ 操作按钮放在复制按钮前面（左侧） —— 与用户要求一致 */}
        {actionLabel && onAction && (
          <button className="hdr-action-btn" onClick={onAction} title={actionLabel}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M23 4v6h-6"/>
              <path d="M1 20v-6h6"/>
              <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10"/>
              <path d="M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
            </svg>
          </button>
        )}
        <button
          className="hdr-copy-btn"
          onClick={handleCopy}
          title={copied ? "已复制" : "复制值"}
        >
          {copied ? (
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M20 6L9 17l-5-5"/>
            </svg>
          ) : (
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
            </svg>
          )}
        </button>
        {entry.note && (
          <button
            className={`hdr-note-icon ${noteOpen ? "active" : ""}`}
            onClick={() => setNoteOpen((v) => !v)}
            title={entry.note}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="10"/>
              <path d="M12 16v-4M12 8h.01"/>
            </svg>
          </button>
        )}
      </div>
      {entry.note && noteOpen && (
        <div className="hdr-note-expanded">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"/>
          </svg>
          <span>{entry.note}</span>
        </div>
      )}
    </div>
  );
}
