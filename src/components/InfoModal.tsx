import type { ReactNode } from "react";

interface InfoModalProps {
  isOpen: boolean;
  title: string;
  icon?: string;
  sections: {
    title?: string;
    content: ReactNode;
    type?: "text" | "code" | "list" | "hint";
  }[];
  confirmText?: string;
  onConfirm: () => void;
  onCancel: () => void;
  extraText?: string;
  onExtra?: () => void;
}

export function InfoModal({
  isOpen,
  title,
  icon = "ℹ️",
  sections,
  confirmText = "确定",
  onConfirm,
  onCancel,
  extraText,
  onExtra,
}: InfoModalProps) {
  if (!isOpen) return null;

  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div className="info-modal" onClick={(e) => e.stopPropagation()}>
        <div className="info-modal-header">
          <div className="info-modal-icon">{icon}</div>
          <h3 className="info-modal-title">{title}</h3>
          <button className="info-modal-close" onClick={onCancel}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="20" height="20">
              <line x1="18" y1="6" x2="6" y2="18"/>
              <line x1="6" y1="6" x2="18" y2="18"/>
            </svg>
          </button>
        </div>

        <div className="info-modal-body">
          {sections.map((section, index) => (
            <div key={index} className="info-section">
              {section.title && <h4 className="info-section-title">{section.title}</h4>}
              {section.type === "code" ? (
                <pre className="info-code-block">
                  <code>{section.content}</code>
                </pre>
              ) : section.type === "list" ? (
                <div className="info-list">{section.content}</div>
              ) : section.type === "hint" ? (
                <div className="info-hint">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="16" height="16">
                    <circle cx="12" cy="12" r="10"/>
                    <path d="M12 16v-4M12 8h.01"/>
                  </svg>
                  <span>{section.content}</span>
                </div>
              ) : (
                <p className="info-text">{section.content}</p>
              )}
            </div>
          ))}
        </div>

        <div className="info-modal-footer">
          {extraText && onExtra && (
            <button className="info-btn extra" onClick={onExtra}>
              {extraText}
            </button>
          )}
          <button className="info-btn cancel" onClick={onCancel}>
            取消
          </button>
          <button className="info-btn confirm" onClick={onConfirm}>
            {confirmText}
          </button>
        </div>
      </div>
    </div>
  );
}
