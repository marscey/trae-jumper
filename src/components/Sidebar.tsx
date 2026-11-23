import { useEffect, useRef, useState } from "react";
import {
  BarChart3,
  Users,
  Settings,
  Info,
  ChevronsLeft,
  ChevronsRight,
  ArrowDownToLine,
} from "lucide-react";
import logoImage from "../assets/logo.png";

interface SidebarProps {
  currentPage: string;
  onNavigate: (page: string) => void;
  /** 有新版本时显示的更新状态（可选） */
  updateStatus?: "available" | "downloading" | "ready";
  /** 更新弹窗图标被点击时 */
  onOpenUpdate?: () => void;
}

const menuItems = [
  { id: "dashboard", label: "仪表盘", icon: BarChart3 },
  { id: "accounts", label: "账号管理", icon: Users },
  { id: "settings", label: "设置", icon: Settings },
  { id: "about", label: "关于", icon: Info },
];

const AUTO_COLLAPSE_WIDTH = 1100;

export function Sidebar({ currentPage, onNavigate, updateStatus, onOpenUpdate }: SidebarProps) {
  const [collapsed, setCollapsed] = useState(() => window.innerWidth < AUTO_COLLAPSE_WIDTH);
  const prevNarrowRef = useRef(window.innerWidth < AUTO_COLLAPSE_WIDTH);

  useEffect(() => {
    const onResize = () => {
      const narrow = window.innerWidth < AUTO_COLLAPSE_WIDTH;
      if (narrow && !prevNarrowRef.current) {
        setCollapsed(true);
      }
      if (!narrow && prevNarrowRef.current) {
        setCollapsed(false);
      }
      prevNarrowRef.current = narrow;
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  const toggleCollapsed = () => setCollapsed((v) => !v);

  const hasUpdateIndicator = updateStatus != null;

  return (
    <aside className={`sidebar ${collapsed ? "collapsed" : ""}`}>
      <div className="sidebar-logo">
        <div className="logo-icon">
          <img src={logoImage} alt="Logo" className="logo-image" />
        </div>
        <span className="logo-text">TraeJumper</span>
      </div>

      <nav className="sidebar-nav">
        {menuItems.map((item) => {
          const Icon = item.icon;
          return (
            <div
              key={item.id}
              className={`sidebar-item ${currentPage === item.id ? "active" : ""}`}
              onClick={() => onNavigate(item.id)}
              title={collapsed ? item.label : undefined}
            >
              <span className="sidebar-icon">
                <Icon />
              </span>
              <span className="sidebar-label">{item.label}</span>
            </div>
          );
        })}
      </nav>

      <div className="sidebar-footer">
        {/* 更新入口图标：有新版本时高亮 + 红点 */}
        {hasUpdateIndicator && onOpenUpdate && (
          <button
            className={`sidebar-item footer-update-indicator ${updateStatus}`}
            onClick={onOpenUpdate}
            title={
              updateStatus === "available"
                ? "有新版本可用，点击查看"
                : updateStatus === "downloading"
                ? "正在下载更新…"
                : "更新已完成，点击重启"
            }
          >
            <span className="sidebar-icon">
              <ArrowDownToLine />
              {updateStatus === "downloading" && (
                <span className="update-spinner-badge" />
              )}
              {updateStatus === "ready" && (
                <span className="update-ready-badge" />
              )}
              {updateStatus === "available" && (
                <span className="update-dot-badge" />
              )}
            </span>
            <span className="sidebar-label">
              {updateStatus === "available"
                ? "新版本可用"
                : updateStatus === "downloading"
                ? "下载中…"
                : "更新完成"}
            </span>
          </button>
        )}

        <div
          className="sidebar-item footer-toggle"
          onClick={toggleCollapsed}
          title={collapsed ? "展开菜单" : "收起菜单"}
        >
          <span className="sidebar-icon">
            {collapsed ? <ChevronsRight size={18} /> : <ChevronsLeft size={18} />}
          </span>
          <span className="sidebar-label">{collapsed ? "展开菜单" : "收起菜单"}</span>
        </div>
        <div className="footer-meta">
          <span className="version">v{__APP_VERSION__}</span>
        </div>
      </div>
    </aside>
  );
}
