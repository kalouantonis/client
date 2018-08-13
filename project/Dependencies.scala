import sbt._
import Keys._

object Dependencies {
  val fs2Version = "0.10.5"

  val all = Seq(
    "org.typelevel" %% "cats-core" % "1.1.0",
    "org.typelevel" %% "cats-effect" % "0.10",
    "co.fs2" %% "fs2-core" % fs2Version,
    "co.fs2" %% "fs2-io" % fs2Version,
    // Audio
    "com.googlecode.soundlibs" % "tritonus-share" % "0.3.7.4" % Runtime,
    "com.googlecode.soundlibs" % "jlayer" % "1.0.1.4" % Runtime,
    "com.googlecode.soundlibs" % "mp3spi" % "1.9.5.4" % Runtime,
    "org.scalatest" %% "scalatest" % "3.0.5" % Test
  )
}