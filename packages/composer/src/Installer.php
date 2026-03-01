<?php

namespace Veld;

/**
 * Composer lifecycle hook — downloads the veld binary during install/update.
 */
class Installer
{
    public static function install(): void
    {
        $binScript = __DIR__ . '/../bin/veld';

        if (!file_exists($binScript)) {
            fwrite(STDERR, "Warning: veld bin script not found at {$binScript}\n");
            return;
        }

        // Run the bin script with --version to trigger download
        $cmd = PHP_BINARY . ' ' . escapeshellarg($binScript) . ' --version 2>&1';
        exec($cmd, $output, $exitCode);

        if ($exitCode === 0) {
            fwrite(STDOUT, "✓ veld binary ready\n");
        } else {
            fwrite(STDERR, "Warning: Could not pre-download veld binary. It will be downloaded on first use.\n");
        }
    }
}

