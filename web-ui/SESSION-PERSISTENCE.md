# Dashboard remembered session

The dashboard does not store the user's plaintext password.

After a successful unlock, the decrypted 256-bit workspace key is remembered in browser `localStorage`, scoped by repository. This allows reloads and browser restarts to restore the dashboard without asking for the password again.

Using the dashboard lock action removes the remembered workspace key immediately. Private-browsing/storage failures fall back to normal password unlock.
