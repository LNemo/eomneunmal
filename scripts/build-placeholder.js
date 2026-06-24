import { mkdirSync, copyFileSync } from 'node:fs';
import { join } from 'node:path';

mkdirSync('dist/src', { recursive: true });
copyFileSync('index.html', 'dist/index.html');
copyFileSync('src/main.js', join('dist', 'src', 'main.js'));
copyFileSync('src/ui-state.js', join('dist', 'src', 'ui-state.js'));
copyFileSync('src/styles.css', join('dist', 'src', 'styles.css'));
console.log('없는말 static frontend copied to dist/.');
