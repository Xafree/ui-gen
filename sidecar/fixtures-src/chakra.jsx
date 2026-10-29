import { createRoot } from 'react-dom/client';
import { ChakraProvider, defaultSystem, Button } from '@chakra-ui/react';

createRoot(document.getElementById('mount')).render(
  <ChakraProvider value={defaultSystem}>
    <Button data-uigen-root>Bouton</Button>
  </ChakraProvider>,
);
