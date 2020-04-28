# Bundle Stage
FROM alpine
ADD ./zobar /zobar
CMD ["/zobar"]
