# Bundle Stage
FROM alpine
ADD ./zotimer /zotimer
CMD ["/zotimer"]
