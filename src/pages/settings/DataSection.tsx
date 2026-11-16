interface DataSectionProps {
  onExport?: () => void;
  onImport?: () => void;
  onClearData?: () => void;
}

/** 设置页 · 数据管理分区：导出 / 导入 / 清空 */
export function DataSection({ onExport, onImport, onClearData }: DataSectionProps) {
  return (
    <div className="settings-section">
      <h3>数据管理</h3>
      <div className="setting-item">
        <div className="setting-info">
          <div className="setting-label">导出数据</div>
          <div className="setting-desc">导出所有账号数据为 JSON 文件</div>
        </div>
        <button className="setting-btn" onClick={onExport}>导出</button>
      </div>

      <div className="setting-item">
        <div className="setting-info">
          <div className="setting-label">导入数据</div>
          <div className="setting-desc">从 JSON 文件导入账号数据</div>
        </div>
        <button className="setting-btn" onClick={onImport}>导入</button>
      </div>

      <div className="setting-item danger">
        <div className="setting-info">
          <div className="setting-label">清空数据</div>
          <div className="setting-desc">删除所有账号数据（不可恢复）</div>
        </div>
        <button className="setting-btn danger" onClick={onClearData}>清空</button>
      </div>
    </div>
  );
}
