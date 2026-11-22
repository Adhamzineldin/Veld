import { createRoot } from 'react-dom/client';
import { Bank } from './Bank';
import { sameOrigin } from './clients';

const root = document.getElementById('root');
if (root) createRoot(root).render(<Bank urls={sameOrigin()} />);
