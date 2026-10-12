import './ui/hud.css';

import { createRoot } from 'react-dom/client';

import { App } from './App.tsx';
// 겉모양 한 겹: 다른 CSS 뒤에 불러온다
import './ui/theme.css';

const root = document.getElementById('root');
if (root) createRoot(root).render(<App />);
