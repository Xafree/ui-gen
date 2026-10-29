import { createRoot } from 'react-dom/client';
import { createTheme, ThemeProvider } from '@mui/material/styles';
import Button from '@mui/material/Button';

const theme = createTheme();

createRoot(document.getElementById('mount')).render(
  <ThemeProvider theme={theme}>
    <Button variant="contained" data-uigen-root>Bouton</Button>
  </ThemeProvider>,
);
