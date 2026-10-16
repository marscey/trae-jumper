import { UsageEvents } from "./UsageEvents";

interface UsageModalProps {
  isOpen: boolean;
  accountId: string;
  accountName: string;
  onClose: () => void;
}

export function UsageModal({ isOpen, accountId, accountName, onClose }: UsageModalProps) {
  if (!isOpen) return null;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content detail-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header-fixed">
          <div className="modal-header-title-wrap">
            <h2>账号用量</h2>
            {accountName && <span className="modal-header-subtitle">{accountName}</span>}
          </div>
          <button className="modal-close-btn" onClick={onClose}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="20" height="20">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        <div className="modal-body-scrollable">
          <UsageEvents accountId={accountId} />
        </div>

        <div className="modal-actions-fixed">
          <button onClick={onClose}>关闭</button>
        </div>
      </div>
    </div>
  );
}
