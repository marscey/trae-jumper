import { useEffect, useRef, useState } from "react";
import { BarChart3, Users, Settings, Info, ChevronsLeft, ChevronsRight } from "lucide-react";
import logoImage from "../assets/logo.png";

interface SidebarProps {
  currentPage: string;
  onNavigate: (page: string) => void;
}

const menuItems = [
  { id: "dashboard", label: "仪表盘", icon: BarChart3 },
  { id: "accounts", label: "账号管理", icon: Users },
  { id: "settings", label: "设置", icon: Settings },
  { id: "about", label: "关于", icon: Info },
];

const AUTO_COLLAPSE_WIDTH = 1100;

export function Sidebar({ currentPage, onNavigate }: SidebarProps) {
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
