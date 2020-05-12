# Remote Tablet

This repo contains a minimal example of how to use your ipad as a remote tablet for linux. There is no need to install anything on the ipad.

## Requirements

For linux the following requirements need to be met.
* rustr
* libgtk-dev
* xdotool


## Usage
Start the app. Scan the QR code with your ipad. And you're set.

## Security Concerns
This implementation had a lot of security problems as the data is sent unencrypted and unauthenticated over a local network. If you can't trust your local network don't use it.