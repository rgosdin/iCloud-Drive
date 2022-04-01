# Hypnos - The selfhosted audio cloud
Hypnos allows you to listen to your music from anywhere in the world,
where you have access to an internet connection.

It uses the embedded tags in your files do determine the audio metadata.
Cover art is supported via external files in the same directory as the songs.

![Screenshot - AlbumList - Desktop](/~serra/Soil/blob/master/img/AlbumList-Desktop.png "Screenshot of the AlbumList for Desktop")
![Screenshot - Album - Desktop](/~serra/Soil/blob/master/img/Album-Desktop.png "Screenshot of an Album for Desktop")
![Screenshot - Album - Mobile](/~serra/Soil/blob/master/img/Album-Mobile.png "Screenshot of an Album for Mobile")

## How it works
Hypnos consists of two parts.

A backend called [Soil](https://git.sr.ht/~serra/Soil). It reads your audio
library on startup and saves the foundsSongs into a Postgresql database.
It provides an API which you can use to get information about your audio
library.
The backend will transparently transcode your songs for playback using `ffmpeg`.

There is also a frontend called [Solanum](https://git.sr.ht/~serra/Solanum).
It runs in the browser and allows you to browse your library and play songs.

## Features
* Transcode songs on the fly.
* Reuse the tags of your songs.
* Support for album art.
* LibreJS compatible.
* Responve UI

### Supported Files
Currently the following files are supported:

Songs:
* .flac
* .mp3
* .wv

Cover Art:
* .png
* .jpg | .jpeg

### Supported Tags
<table>
	<thead>
		<tr>
			<th>Field</th>
			<th>Tags</th>
			<th>Required</th>
		</tr>
	</thead>
	<tbody>
		<tr>
			<td>Album Artist</td>
			<td>album artist, albumartist, album_artist</td>
			<td>Yes</td>
		</tr>
		<tr>
			<td>Album Title</td>
			<td>album</td>
			<td>Yes</td>
		</tr>
		<tr>
			<td>Song Title</td>
			<td>title</td>
			<td>Yes</td>
		</tr>
		<tr>
			<td>Track Number</td>
			<td>track, tracknumber</td>
			<td>Yes</td>
		</tr>
		<tr>
			<td>Disc</td>
			<td>disc</td>
			<td>No</td>
		</tr>
	</tbody>
</table>

## How to install
### Packages
On Archlinux you can use the
[hypnos-soil](https://aur.archlinux.org/packages/hypnos-soil/) AUR package.

### Set up Soil
Checkout [Soil](https://git.sr.ht/~serra/Soil) and compile it with
`cargo build --release`. This should result in a binary at
`target/release/soil`. Soil is configured entirely with environment variables:

<table>
	<thead>
		<tr>
			<th>Parameter</th>
			<th>Info</th>
		</tr>
	</thead>
	<tbody>
		<tr>
			<td>DB_HOST</td>
			<td>The host of the Postgresql DB (e.g. localhost)</td>
		</tr>
		<tr>
			<td>DB_USER</td>
			<td>Which user to connect to the DB</td>
		</tr>
		<tr>
			<td>DB_PW</td>
			<td>Which password to use to connect to the DB</td>
		</tr>
		<tr>
			<td>DB_NAME</td>
			<td>Which Database to connect to</td>
		</tr>
		<tr>
			<td>LISTEN</td>
			<td>On which IP address and port to listen. Use 0.0.0.0 to listen on all interfaces</td>
		</tr>
		<tr>
			<td>MUSIC_LIB</td>
			<td>Path to the folder containing your Songs (e.g. /srv/hypnos/music)</td>
		</tr>
	</tbody>
</table>

#### Use with systemd
You might want to use `systemd` to launch soil. The author uses the following unit file:
```
# /etc/systemd/system/soil.service
[Unit]
Description=Soil - Hypnos backend
After=network.target

[Service]
User=soil
EnvironmentFile=/etc/conf.d/soil
ExecStart=/usr/bin/soil

[Install]
WantedBy=multi-user.target
```

### Set up Solanum
Please refer to the [Solanum documentation](https://git.sr.ht/~serra/Solanum)

## Security
Soil  by itself does not support https or access management. You might want to use a reverse proxy to limit access. The author uses an `httpd` configuration like the following
```
<VirtualHost _default_:443>
	ServerName your.domain.com

	SSLEngine On
	SSLCertificateFile /path/to/SSLCert
	SSLCertificateKeyFile /path/to/SSLKey

	# Serves the Solanum frontend
	DocumentRoot "/srv/http/solanum"
	<Directory /srv/http/solanum>
		AuthName "Hypnos - Come and chill"
		AuthUserFile "/etc/httpd/conf/users"
		AuthType Basic
		Header always unset Strict-Transport-Security
		Require valid-user granted
		AllowOverride None
	</Directory>

	# Serves the Soil backend
	<LocationMatch "/(albums|song/.*|cover/.*)$">
		AuthName "Hypnos - Come and chill"
		AuthUserFile "/etc/httpd/conf/users"
		AuthType Basic
		Require valid-user granted
		ProxyPassMatch http://localhost:3030/$1
		ProxyPassReverse http://localhost:3030/$1
	</LocationMatch>
</VirtualHost>
```

## Further information
If you encounter any bugs in either Soil or Solanum please create a ticket
at https://todo.sr.ht/~serra/hypnos. You can also add feature requests there.

If you have any improvements, questions or suggestions that don't seem fit for
the bugtracker please write a mail to hypnos@peterlamby.de.

If you want to contribute code please send your patches to
hypnos@peterlamby.de. You can either use
[git-send-email](https://git-send-email.io/) to send the patches directly
or just host your patches somewhere and send me a pull request via email.
